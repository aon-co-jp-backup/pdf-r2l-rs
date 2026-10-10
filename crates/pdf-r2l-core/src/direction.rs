//! 綴じ方向(右綴じ/左綴じ)と見開き表示の設定。
//!
//! PDF の仕様書(ISO 32000-1)だけを根拠に書いている。
//! - 12.2 Viewer Preferences: 文書カタログの `/ViewerPreferences` 辞書の `/Direction`
//!   (`/L2R` = 左から右〈既定〉、`/R2L` = 右から左)。
//! - 7.7.2 Document Catalog: 文書カタログの `/PageLayout`
//!   (`/TwoColumnLeft` / `/TwoColumnRight` = 2列の見開きで奇数ページが左/右、など)。

use lopdf::{Dictionary, Document, Object, ObjectId};

/// 綴じ方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    /// 右綴じ(右から左へ読む。日本語の縦書きの本など)。
    RightToLeft,
    /// 左綴じ(左から右へ読む)。
    LeftToRight,
    /// 元と同じ(綴じ方向にも見開きにも触れない。OCR だけを使いたいとき)。
    Keep,
}

impl Binding {
    /// `/Direction` の値(元と同じ なら無し)。
    fn direction_name(self) -> Option<&'static str> {
        match self {
            Binding::RightToLeft => Some("R2L"),
            Binding::LeftToRight => Some("L2R"),
            Binding::Keep => None,
        }
    }

    /// 見開き表示のときの `/PageLayout` の値(表紙などの奇数ページが、右綴じなら右・左綴じなら左に来る)。
    fn spread_layout_name(self) -> Option<&'static str> {
        match self {
            Binding::RightToLeft => Some("TwoColumnRight"),
            Binding::LeftToRight => Some("TwoColumnLeft"),
            Binding::Keep => None,
        }
    }
}

/// 文書に書き込む設定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectionOptions {
    /// 綴じ方向。
    pub binding: Binding,
    /// 見開き表示(2列)にするか。偽なら `/PageLayout` には触れない。
    pub spread: bool,
}

fn name(s: &str) -> Object {
    Object::Name(s.as_bytes().to_vec())
}

fn catalog_id(doc: &Document) -> Result<ObjectId, String> {
    match doc.trailer.get(b"Root") {
        Ok(Object::Reference(id)) => Ok(*id),
        _ => Err("文書カタログが見つかりません(壊れた PDF の可能性) / Document catalog not found (the PDF may be damaged)".to_string()),
    }
}

/// `/ViewerPreferences` の置き場所。間接参照なら参照先のオブジェクト、そうでなければカタログ内の直接辞書。
fn viewer_preferences_mut(doc: &mut Document, catalog: ObjectId) -> Result<&mut Dictionary, String> {
    let referenced = match doc.get_dictionary(catalog) {
        Ok(cat) => match cat.get(b"ViewerPreferences") {
            Ok(Object::Reference(id)) => Some(*id),
            _ => None,
        },
        Err(e) => return Err(e.to_string()),
    };
    match referenced {
        Some(id) => doc.get_dictionary_mut(id).map_err(|e| e.to_string()),
        None => {
            let cat = doc.get_dictionary_mut(catalog).map_err(|e| e.to_string())?;
            if !matches!(cat.get(b"ViewerPreferences"), Ok(Object::Dictionary(_))) {
                cat.set("ViewerPreferences", Object::Dictionary(Dictionary::new()));
            }
            match cat.get_mut(b"ViewerPreferences") {
                Ok(Object::Dictionary(d)) => Ok(d),
                _ => Err("ViewerPreferences を作成できません / Cannot create ViewerPreferences".to_string()),
            }
        }
    }
}

/// 読み込み済みの文書に設定を書き込む(他の `/ViewerPreferences` の項目は保持する)。
pub fn apply_to_document(doc: &mut Document, opts: DirectionOptions) -> Result<(), String> {
    // 元と同じ なら、何も書き換えない。
    let Some(direction) = opts.binding.direction_name() else {
        return Ok(());
    };
    let catalog = catalog_id(doc)?;
    viewer_preferences_mut(doc, catalog)?.set("Direction", name(direction));
    if let (true, Some(layout)) = (opts.spread, opts.binding.spread_layout_name()) {
        doc.get_dictionary_mut(catalog).map_err(|e| e.to_string())?.set("PageLayout", name(layout));
    }
    Ok(())
}

/// PDF のバイト列に設定を書き込んで、新しいバイト列を返す。
pub fn apply(pdf: &[u8], opts: DirectionOptions) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(pdf)
        .map_err(|e| format!("PDFを読み込めません(暗号化・破損の可能性) / Cannot read the PDF (encrypted or damaged?): {e}"))?;
    apply_to_document(&mut doc, opts)?;
    let mut out = Vec::new();
    doc.save_to(&mut out).map_err(|e| format!("保存に失敗しました / Save failed: {e}"))?;
    Ok(out)
}

/// 文書に書かれている綴じ方向(無ければ `None` = 既定の左綴じ)。表示・検証用。
pub fn read_binding(pdf: &[u8]) -> Result<Option<Binding>, String> {
    let doc = Document::load_mem(pdf).map_err(|e| e.to_string())?;
    let catalog = catalog_id(&doc)?;
    let cat = doc.get_dictionary(catalog).map_err(|e| e.to_string())?;
    let prefs = match cat.get(b"ViewerPreferences") {
        Ok(Object::Reference(id)) => doc.get_dictionary(*id).ok(),
        Ok(Object::Dictionary(d)) => Some(d),
        _ => None,
    };
    Ok(prefs.and_then(|p| p.get(b"Direction").ok()).and_then(|d| d.as_name().ok()).and_then(|n| match n {
        b"R2L" => Some(Binding::RightToLeft),
        b"L2R" => Some(Binding::LeftToRight),
        _ => None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    /// 1ページだけの最小の PDF。`prefs` が `Some` なら、その辞書を `/ViewerPreferences` として(直接または間接で)付ける。
    pub(crate) fn minimal_pdf(prefs: Option<(Dictionary, bool)>) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages = doc.new_object_id();
        let page = doc.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()]});
        doc.objects.insert(pages, Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}));
        let mut cat = dictionary! {"Type"=>"Catalog","Pages"=>pages};
        if let Some((d, indirect)) = prefs {
            if indirect {
                let id = doc.add_object(d);
                cat.set("ViewerPreferences", Object::Reference(id));
            } else {
                cat.set("ViewerPreferences", Object::Dictionary(d));
            }
        }
        let cat = doc.add_object(cat);
        doc.trailer.set("Root", cat);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap_or_default();
        out
    }

    fn opts(binding: Binding, spread: bool) -> DirectionOptions {
        DirectionOptions { binding, spread }
    }

    #[test]
    fn keep_changes_nothing_even_with_spread() {
        let src = minimal_pdf(Some((dictionary! {"HideToolbar"=>true}, false)));
        let out = apply(&src, opts(Binding::Keep, true)).unwrap_or_default();
        assert_eq!(read_binding(&out), Ok(None));
        assert_eq!(page_layout(&out), None);
    }

    fn page_layout(pdf: &[u8]) -> Option<Vec<u8>> {
        let doc = Document::load_mem(pdf).ok()?;
        let cat = doc.get_dictionary(catalog_id(&doc).ok()?).ok()?;
        cat.get(b"PageLayout").ok()?.as_name().ok().map(|n| n.to_vec())
    }

    #[test]
    fn sets_and_reads_direction() {
        let out = apply(&minimal_pdf(None), opts(Binding::RightToLeft, false)).unwrap_or_default();
        assert_eq!(read_binding(&out), Ok(Some(Binding::RightToLeft)));
        let back = apply(&out, opts(Binding::LeftToRight, false)).unwrap_or_default();
        assert_eq!(read_binding(&back), Ok(Some(Binding::LeftToRight)));
        assert_eq!(page_layout(&back), None, "layout untouched when spread is off");
    }

    #[test]
    fn spread_sets_page_layout_by_binding() {
        let r = apply(&minimal_pdf(None), opts(Binding::RightToLeft, true)).unwrap_or_default();
        assert_eq!(page_layout(&r).as_deref(), Some(&b"TwoColumnRight"[..]));
        let l = apply(&minimal_pdf(None), opts(Binding::LeftToRight, true)).unwrap_or_default();
        assert_eq!(page_layout(&l).as_deref(), Some(&b"TwoColumnLeft"[..]));
    }

    #[test]
    fn keeps_other_viewer_preferences_direct_and_indirect() {
        for indirect in [false, true] {
            let prefs = dictionary! {"HideToolbar"=>true,"Direction"=>"L2R"};
            let out = apply(&minimal_pdf(Some((prefs, indirect))), opts(Binding::RightToLeft, false)).unwrap_or_default();
            assert_eq!(read_binding(&out), Ok(Some(Binding::RightToLeft)));
            let doc = Document::load_mem(&out).unwrap_or_default();
            let cat = doc.get_dictionary(catalog_id(&doc).unwrap_or((0, 0))).unwrap_or(&Dictionary::new()).clone();
            let p = match cat.get(b"ViewerPreferences") {
                Ok(Object::Reference(id)) => doc.get_dictionary(*id).cloned().unwrap_or_default(),
                Ok(Object::Dictionary(d)) => d.clone(),
                _ => Dictionary::new(),
            };
            assert_eq!(p.get(b"HideToolbar").ok().and_then(|o| o.as_bool().ok()), Some(true), "indirect={indirect}");
        }
    }

    #[test]
    fn rejects_garbage() {
        assert!(apply(b"not a pdf", opts(Binding::RightToLeft, false)).is_err());
    }
}
