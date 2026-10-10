//! 1回の変換作業: (OCR・テキストフォント文字化) → 綴じ方向・見開き → 保存。
//! 保存は一時ファイルに書いてから置き換えるので、途中で失敗しても出力先や元のファイルを壊さない。

use crate::direction::{apply, Binding, DirectionOptions};
use open_runo_pdf_ocr::{process, Progress, Report, Settings};
use std::path::{Path, PathBuf};

/// 保存の仕方。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    /// 別のファイルへ保存する。
    NewFile(PathBuf),
    /// 元のファイルを上書きする。`backup` が真なら、先に `<元の名前>.bak` へ退避する。
    InPlace {
        /// 上書き前にバックアップを作るか。
        backup: bool,
    },
}

/// 変換作業の指定。
#[derive(Debug, Clone)]
pub struct Task {
    /// 元の PDF。
    pub source: PathBuf,
    /// 保存の仕方。
    pub output: Output,
    /// 綴じ方向・見開き。
    pub direction: DirectionOptions,
    /// `Some` かつ `enabled` なら OCR・テキストフォント文字化をかける。
    pub ocr: Option<Settings>,
}

/// 変換の結果。
#[derive(Debug, Clone)]
pub struct Outcome {
    /// 保存したファイル。
    pub destination: PathBuf,
    /// 日英の結果メッセージ。
    pub message: String,
    /// OCR の概要(OCR をかけたとき)。
    pub report: Option<Report>,
}

/// 保存先の名前の案: `本.pdf` → `本-r2l.pdf` / `本-l2r.pdf`。
pub fn suggest_output_name(source: &Path, binding: Binding) -> String {
    let stem = source.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "output".to_string());
    let suffix = match binding {
        Binding::RightToLeft => "r2l",
        Binding::LeftToRight => "l2r",
        Binding::Keep => "ocr",
    };
    format!("{stem}-{suffix}.pdf")
}

fn describe(report: &Report) -> String {
    format!(
        "OCR: {}ページ・{}行を認識。{}行を文字に置き換え、{}行は元の画像のまま検索用の見えない文字を追加しました。\n\
         OCR: read {} lines on {} pages; replaced {} lines with text; kept the original image for {} lines and added invisible searchable text.",
        report.pages, report.lines_found, report.lines_retyped, report.lines_text_only,
        report.lines_found, report.pages, report.lines_retyped, report.lines_text_only,
    )
}

/// 一時ファイルに書いてから名前を付け替える。
fn write_atomically(dest: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = dest.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| format!("フォルダを作れません / Cannot create the folder: {e}"))?;
    }
    let mut tmp = dest.as_os_str().to_owned();
    tmp.push(".tmp-pdf-r2l");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes).map_err(|e| format!("保存に失敗しました / Save failed: {e}"))?;
    std::fs::rename(&tmp, dest).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("保存に失敗しました / Save failed: {e}")
    })
}

/// 既存のバックアップを上書きしない名前(`x.pdf.bak`、`x.pdf.bak1`、…)を選ぶ。
fn backup_path(source: &Path) -> PathBuf {
    let mut base = source.as_os_str().to_owned();
    base.push(".bak");
    let first = PathBuf::from(&base);
    if !first.exists() {
        return first;
    }
    (1..1000)
        .map(|n| {
            let mut p = base.clone();
            p.push(n.to_string());
            PathBuf::from(p)
        })
        .find(|p| !p.exists())
        .unwrap_or(first)
}

/// ファイルを介さず、PDF のバイト列を変換する(Android など)。OCR(有効なら) → 綴じ方向・見開き の順。
/// 何も変更しない指定(元と同じ + OCR なし)はエラーにする。
pub fn transform(
    input: &[u8],
    direction: DirectionOptions,
    ocr: Option<&Settings>,
    cache_dir: &Path,
    progress: &mut dyn FnMut(Progress),
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, Option<Report>), String> {
    let ocr = ocr.filter(|s| s.enabled);
    if direction.binding == Binding::Keep && ocr.is_none() {
        return Err("変更する内容がありません。綴じ方向を選ぶか、OCR をオンにしてください。
Nothing to change. Choose a binding direction, or turn OCR on.".to_string());
    }
    let (mut bytes, mut report) = (input.to_vec(), None);
    if let Some(settings) = ocr {
        let (out, r) = process(&bytes, settings, cache_dir, progress, cancelled)?;
        bytes = out;
        report = Some(r);
    }
    if direction.binding != Binding::Keep {
        bytes = apply(&bytes, direction)?;
    }
    Ok((bytes, report))
}

/// 結果の日英メッセージ(OCR の概要つき)。
pub fn summarize(destination: &str, report: Option<&Report>) -> String {
    let mut message = format!("完了 / Done: {destination}");
    if let Some(r) = report {
        message.push('\n');
        message.push_str(&describe(r));
        if r.lines_text_only * 3 > r.lines_found && r.lines_found > 0 {
            message.push_str(
                "
(置き換えなかった行が多いです。言語の選択が文書と合っているか確認してください。)
(Many lines were not replaced. Check that the selected languages match the document.)",
            );
        }
    }
    message
}

/// 変換を実行する。`progress` で進捗、`cancelled` でキャンセルを受け付ける。
pub fn run(task: &Task, cache_dir: &Path, progress: &mut dyn FnMut(Progress), cancelled: &dyn Fn() -> bool) -> Result<Outcome, String> {
    let destination = match &task.output {
        Output::NewFile(p) => {
            if p == &task.source {
                return Err("元のファイルと同じ場所には保存できません。別の名前にするか「上書き」を選んでください。\nCannot save over the original here. Choose another name, or choose \"overwrite\".".to_string());
            }
            p.clone()
        }
        Output::InPlace { .. } => task.source.clone(),
    };
    let ocr_on = task.ocr.as_ref().is_some_and(|s| s.enabled);
    if task.direction.binding == Binding::Keep && !ocr_on {
        return Err("変更する内容がありません。綴じ方向を選ぶか、OCR をオンにしてください。
Nothing to change. Choose a binding direction, or turn OCR on.".to_string());
    }
    let original = std::fs::read(&task.source).map_err(|e| format!("読み込みに失敗しました / Read failed: {e}"))?;

    let (bytes, report) = transform(&original, task.direction, task.ocr.as_ref(), cache_dir, progress, cancelled)?;

    if let Output::InPlace { backup: true } = &task.output {
        let bak = backup_path(&task.source);
        std::fs::write(&bak, &original).map_err(|e| format!("バックアップを作れません / Cannot write the backup: {e}"))?;
    }
    write_atomically(&destination, &bytes)?;

    let message = summarize(&destination.display().to_string(), report.as_ref());
    Ok(Outcome { destination, message, report })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::direction::read_binding;

    fn minimal() -> Vec<u8> {
        use lopdf::{dictionary, Document, Object};
        let mut doc = Document::with_version("1.5");
        let pages = doc.new_object_id();
        let page = doc.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()]});
        doc.objects.insert(pages, Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}));
        let cat = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
        doc.trailer.set("Root", cat);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap_or_default();
        out
    }

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pdf-r2l-core-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap_or_default();
        d
    }

    fn opts() -> DirectionOptions {
        DirectionOptions { binding: Binding::RightToLeft, spread: false }
    }

    #[test]
    fn writes_a_new_file_and_leaves_the_original() {
        let d = dir("new");
        let (src, out) = (d.join("a.pdf"), d.join("out").join("b.pdf"));
        std::fs::write(&src, minimal()).unwrap_or_default();
        let task = Task { source: src.clone(), output: Output::NewFile(out.clone()), direction: opts(), ocr: None };
        let o = run(&task, &d, &mut |_| {}, &|| false).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(o.destination, out);
        assert_eq!(read_binding(&std::fs::read(&out).unwrap_or_default()), Ok(Some(Binding::RightToLeft)));
        assert_eq!(read_binding(&std::fs::read(&src).unwrap_or_default()), Ok(None), "the original is untouched");
        assert!(!d.join("out").join("b.pdf.tmp-pdf-r2l").exists());
    }

    #[test]
    fn refuses_to_overwrite_the_source_as_a_new_file() {
        let d = dir("same");
        let src = d.join("a.pdf");
        std::fs::write(&src, minimal()).unwrap_or_default();
        let task = Task { source: src.clone(), output: Output::NewFile(src), direction: opts(), ocr: None };
        assert!(run(&task, &d, &mut |_| {}, &|| false).is_err());
    }

    #[test]
    fn in_place_makes_unique_backups() {
        let d = dir("inplace");
        let src = d.join("a.pdf");
        let original = minimal();
        std::fs::write(&src, &original).unwrap_or_default();
        let task = Task { source: src.clone(), output: Output::InPlace { backup: true }, direction: opts(), ocr: None };
        assert!(run(&task, &d, &mut |_| {}, &|| false).is_ok());
        assert_eq!(std::fs::read(d.join("a.pdf.bak")).unwrap_or_default(), original);
        assert_eq!(read_binding(&std::fs::read(&src).unwrap_or_default()), Ok(Some(Binding::RightToLeft)));
        assert!(run(&task, &d, &mut |_| {}, &|| false).is_ok());
        assert!(d.join("a.pdf.bak1").exists(), "the first backup must not be overwritten");
    }

    #[test]
    fn failure_leaves_no_output_and_keeps_the_source() {
        let d = dir("fail");
        let (src, out) = (d.join("a.pdf"), d.join("b.pdf"));
        std::fs::write(&src, minimal()).unwrap_or_default();
        let ocr = Settings { server_url: "http://127.0.0.1:1".into(), ..Settings::default() };
        let task = Task { source: src.clone(), output: Output::NewFile(out.clone()), direction: opts(), ocr: Some(ocr) };
        let err = run(&task, &d, &mut |_| {}, &|| false).err().unwrap_or_default();
        assert!(err.contains("オンライン環境が必要") && err.contains("online environment"), "{err}");
        assert!(!out.exists());
        assert_eq!(read_binding(&std::fs::read(&src).unwrap_or_default()), Ok(None));
    }

    #[test]
    fn disabled_ocr_never_touches_the_network() {
        let d = dir("disabled");
        let (src, out) = (d.join("a.pdf"), d.join("b.pdf"));
        std::fs::write(&src, minimal()).unwrap_or_default();
        let ocr = Settings { enabled: false, server_url: "http://127.0.0.1:1".into(), ..Settings::default() };
        let task = Task { source: src, output: Output::NewFile(out.clone()), direction: opts(), ocr: Some(ocr) };
        assert!(run(&task, &d, &mut |_| {}, &|| false).is_ok());
        assert!(out.exists());
    }

    #[test]
    fn suggests_names() {
        assert_eq!(suggest_output_name(Path::new("C:/x/本.pdf"), Binding::RightToLeft), "本-r2l.pdf");
        assert_eq!(suggest_output_name(Path::new("a.pdf"), Binding::LeftToRight), "a-l2r.pdf");
    }
}

#[cfg(test)]
mod keep_tests {
    use super::*;
    use crate::direction::{DirectionOptions, read_binding};

    #[test]
    fn keep_without_ocr_is_rejected_and_names_get_an_ocr_suffix() {
        let d = std::env::temp_dir().join("pdf-r2l-core-keep");
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap_or_default();
        let src = d.join("a.pdf");
        std::fs::write(&src, b"%PDF-1.4").unwrap_or_default();
        let task = Task { source: src.clone(), output: Output::NewFile(d.join("b.pdf")), direction: DirectionOptions { binding: Binding::Keep, spread: true }, ocr: None };
        let err = run(&task, &d, &mut |_| {}, &|| false).err().unwrap_or_default();
        assert!(err.contains("Nothing to change"), "{err}");
        assert_eq!(suggest_output_name(&src, Binding::Keep), "a-ocr.pdf");
        let _ = read_binding(b"");
    }
}
