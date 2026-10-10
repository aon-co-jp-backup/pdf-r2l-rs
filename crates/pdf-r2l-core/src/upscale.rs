//! 写真・画像の高解像度化(アップコンバート)。共有クレート `open-runo-sr`(CPU の AI 超解像 + 高品質な補間)を使う。
//! 目標は フルHD / QHD / 4K / 5K / 8K / 16K の枠(縦横比を保って枠に収める)。

use open_runo_sr::{upscale, Format, Options, Progress, Target};
use std::path::{Path, PathBuf};

pub use open_runo_sr::{output_size, Format as ImageFormat, Target as ImageTarget};

/// 出力ファイル名に付ける印(`写真.jpg` → `写真-4k.png`)。
pub fn target_suffix(t: Target) -> &'static str {
    match t {
        Target::FullHd => "fhd",
        Target::Qhd => "qhd",
        Target::Uhd4k => "4k",
        Target::Uhd5k => "5k",
        Target::Uhd8k => "8k",
        Target::Uhd16k => "16k",
    }
}

/// 出力先の名前の案。
pub fn suggest_image_name(source: &Path, target: Target, format: Format) -> String {
    let stem = source.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "image".to_string());
    let ext = match format {
        Format::Png => "png",
        Format::Jpeg => "jpg",
    };
    format!("{stem}-{}.{ext}", target_suffix(target))
}

/// 画像のバイト列を高解像度化する(Android など、ファイルを介さない用途)。
pub fn upscale_bytes(input: &[u8], opts: Options, progress: &mut dyn FnMut(Progress), cancelled: &dyn Fn() -> bool) -> Result<(Vec<u8>, u32, u32, bool), String> {
    let o = upscale(input, opts, progress, cancelled)?;
    Ok((o.bytes, o.width, o.height, o.used_ai))
}

/// 1枚ぶんの結果。
#[derive(Debug, Clone)]
pub struct ImageOutcome {
    /// 保存したファイル。
    pub destination: PathBuf,
    /// 出力の大きさ。
    pub width: u32,
    /// 出力の高さ。
    pub height: u32,
    /// AI 超解像を使ったか。
    pub used_ai: bool,
}

/// 複数の画像を順に高解像度化して `out_dir` に保存する。`progress(全体の進み具合, 説明)` と `cancelled` を受け付ける。
/// 元のファイルは変更しない。同じ名前のファイルがあれば上書きせず、番号を付ける。
pub fn upscale_files(
    inputs: &[PathBuf],
    out_dir: &Path,
    opts: Options,
    progress: &mut dyn FnMut(f32, String),
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<ImageOutcome>, String> {
    std::fs::create_dir_all(out_dir).map_err(|e| format!("フォルダを作れません / Cannot create the folder: {e}"))?;
    let mut done = Vec::new();
    let n = inputs.len().max(1) as f32;
    for (i, path) in inputs.iter().enumerate() {
        if cancelled() {
            return Err("キャンセルしました / Cancelled".to_string());
        }
        let name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let bytes = std::fs::read(path).map_err(|e| format!("読み込みに失敗しました / Read failed ({name}): {e}"))?;
        let base = i as f32 / n;
        let out = upscale(&bytes, opts, &mut |p| progress(base + p.fraction / n, format!("{name}: {}", p.stage)), cancelled).map_err(|e| format!("{name}: {e}"))?;
        let mut dest = out_dir.join(suggest_image_name(path, opts.target, opts.format));
        let mut k = 1;
        while dest.exists() {
            let stem = dest.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let ext = dest.extension().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            dest = out_dir.join(format!("{stem}-{k}.{ext}"));
            k += 1;
        }
        let mut tmp = dest.as_os_str().to_owned();
        tmp.push(".tmp-pdf-r2l");
        let tmp = PathBuf::from(tmp);
        std::fs::write(&tmp, &out.bytes).map_err(|e| format!("保存に失敗しました / Save failed: {e}"))?;
        std::fs::rename(&tmp, &dest).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("保存に失敗しました / Save failed: {e}")
        })?;
        done.push(ImageOutcome { destination: dest, width: out.width, height: out.height, used_ai: out.used_ai });
    }
    progress(1.0, "完了 / Done".to_string());
    Ok(done)
}

/// 結果の日英メッセージ。
pub fn summarize_images(outcomes: &[ImageOutcome]) -> String {
    let mut s = format!("完了 / Done: {}枚 / {} image(s)", outcomes.len(), outcomes.len());
    for o in outcomes {
        s.push_str(&format!("\n  {} ({}×{}{})", o.destination.display(), o.width, o.height, if o.used_ai { ", AI" } else { "" }));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_png(path: &Path) {
        let img = image::RgbImage::from_fn(32, 18, |x, y| image::Rgb([(x * 8) as u8, (y * 14) as u8, 90]));
        img.save(path).unwrap_or_default();
    }

    #[test]
    fn names_and_unique_destinations() {
        assert_eq!(suggest_image_name(Path::new("C:/p/写真.jpg"), Target::Uhd4k, Format::Png), "写真-4k.png");
        assert_eq!(suggest_image_name(Path::new("a.png"), Target::Uhd16k, Format::Jpeg), "a-16k.jpg");
    }

    #[test]
    fn upscales_files_without_touching_originals_and_never_overwrites() {
        let d = std::env::temp_dir().join("pdf-r2l-core-upscale");
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap_or_default();
        let src = d.join("p.png");
        tiny_png(&src);
        let before = std::fs::read(&src).unwrap_or_default();
        let opts = Options { target: Target::FullHd, ai: false, format: Format::Jpeg, threads: 1 };
        let a = upscale_files(&[src.clone()], &d.join("out"), opts, &mut |_, _| {}, &|| false).unwrap_or_else(|e| panic!("{e}"));
        let b = upscale_files(&[src.clone()], &d.join("out"), opts, &mut |_, _| {}, &|| false).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((a[0].width, a[0].height), (1920, 1080));
        assert_ne!(a[0].destination, b[0].destination, "the second run must not overwrite the first");
        assert_eq!(std::fs::read(&src).unwrap_or_default(), before);
        let cancelled = upscale_files(&[src], &d.join("out"), opts, &mut |_, _| {}, &|| true);
        assert!(cancelled.is_err());
    }

    #[test]
    fn a_broken_file_is_reported_with_its_name() {
        let d = std::env::temp_dir().join("pdf-r2l-core-upscale2");
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap_or_default();
        let bad = d.join("bad.png");
        std::fs::write(&bad, b"nope").unwrap_or_default();
        let opts = Options { target: Target::FullHd, ai: false, format: Format::Png, threads: 1 };
        let err = upscale_files(&[bad], &d.join("out"), opts, &mut |_, _| {}, &|| false).err().unwrap_or_default();
        assert!(err.contains("bad.png"), "{err}");
    }
}

/// 画像の大きさ(幅, 高さ)をヘッダーだけから読む(画面の表示用)。
pub fn image_dimensions(path: &Path) -> Result<(u32, u32), String> {
    image::image_dimensions(path).map_err(|e| format!("{}: {e}", path.display()))
}
