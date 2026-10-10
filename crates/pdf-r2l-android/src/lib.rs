//! Android 用の JNI 橋渡し。画面(Kotlin)はファイル選択と設定画面を担当し、PDF の変換・OCR は中核ライブラリ(Rust)が行う。
//! 受け渡しは、PDF のバイト列と JSON 文字列だけ。

use jni::objects::{JByteArray, JClass, JString};
use jni::sys::jstring;
use jni::JNIEnv;
use pdf_r2l_core::ocr::languages::{FEATURE_NOTE_EN, FEATURE_NOTE_JA, LANGUAGES, LANGUAGE_HELP_EN, LANGUAGE_HELP_JA, QUALITY_HELP_EN, QUALITY_HELP_JA};
use pdf_r2l_core::ocr::{OcrClient, Progress, Settings, Stage};
use pdf_r2l_core::{summarize, transform, Binding, DirectionOptions};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static CANCEL: AtomicBool = AtomicBool::new(false);
static PROGRESS: Mutex<String> = Mutex::new(String::new());
static MESSAGE: Mutex<String> = Mutex::new(String::new());

/// Kotlin から渡される設定。
#[derive(Deserialize)]
struct Prefs {
    /// "r2l" / "l2r" / "keep"。
    binding: String,
    spread: bool,
    ocr: Settings,
}

fn set(slot: &Mutex<String>, v: String) {
    if let Ok(mut s) = slot.lock() {
        *s = v;
    }
}

fn get(slot: &Mutex<String>) -> String {
    slot.lock().map(|s| s.clone()).unwrap_or_default()
}

/// 変換の本体(JNI から独立していて、試験できる)。
pub fn convert_bytes(input: &[u8], prefs_json: &str, cache_dir: &str) -> Result<Vec<u8>, String> {
    let prefs: Prefs = serde_json::from_str(prefs_json).map_err(|e| format!("invalid settings: {e}"))?;
    let binding = match prefs.binding.as_str() {
        "l2r" => Binding::LeftToRight,
        "keep" => Binding::Keep,
        _ => Binding::RightToLeft,
    };
    CANCEL.store(false, Ordering::Relaxed);
    set(&PROGRESS, String::new());
    let (out, report) = transform(
        input,
        DirectionOptions { binding, spread: prefs.spread },
        Some(&prefs.ocr),
        &PathBuf::from(cache_dir),
        &mut |p: Progress| {
            let stage = match p.stage {
                Stage::Prepare => "prepare",
                Stage::Ocr => "ocr",
                Stage::Compose => "compose",
            };
            set(&PROGRESS, format!("{stage}|{}|{}", p.done, p.total));
        },
        &|| CANCEL.load(Ordering::Relaxed),
    )?;
    set(&MESSAGE, summarize("", report.as_ref()));
    Ok(out)
}

fn to_jstring(env: &mut JNIEnv, s: &str) -> jstring {
    env.new_string(s).map(|j| j.into_raw()).unwrap_or(std::ptr::null_mut())
}

fn throw(env: &mut JNIEnv, msg: &str) {
    let _ = env.throw_new("java/lang/RuntimeException", msg);
}

/// `external fun convert(input: ByteArray, prefsJson: String, cacheDir: String): ByteArray`
#[no_mangle]
pub extern "system" fn Java_tokyo_aon_pdfr2lrs_Native_convert<'l>(mut env: JNIEnv<'l>, _c: JClass<'l>, input: JByteArray<'l>, prefs: JString<'l>, cache: JString<'l>) -> JByteArray<'l> {
    let null = || unsafe { JByteArray::from_raw(std::ptr::null_mut()) };
    let (Ok(bytes), Ok(prefs), Ok(cache)) = (env.convert_byte_array(&input), env.get_string(&prefs), env.get_string(&cache)) else {
        throw(&mut env, "invalid arguments");
        return null();
    };
    let (prefs, cache): (String, String) = (prefs.into(), cache.into());
    match convert_bytes(&bytes, &prefs, &cache) {
        Ok(out) => env.byte_array_from_slice(&out).unwrap_or_else(|_| null()),
        Err(msg) => {
            throw(&mut env, &msg);
            null()
        }
    }
}

/// 進捗 "stage|done|total"(まだなら空)。
#[no_mangle]
pub extern "system" fn Java_tokyo_aon_pdfr2lrs_Native_progress<'l>(mut env: JNIEnv<'l>, _c: JClass<'l>) -> jstring {
    to_jstring(&mut env, &get(&PROGRESS))
}

/// 直前の変換の結果メッセージ(日英)。
#[no_mangle]
pub extern "system" fn Java_tokyo_aon_pdfr2lrs_Native_message<'l>(mut env: JNIEnv<'l>, _c: JClass<'l>) -> jstring {
    to_jstring(&mut env, &get(&MESSAGE))
}

/// 実行中の変換をキャンセルする。
#[no_mangle]
pub extern "system" fn Java_tokyo_aon_pdfr2lrs_Native_cancel<'l>(_env: JNIEnv<'l>, _c: JClass<'l>) {
    CANCEL.store(true, Ordering::Relaxed);
}

/// 既定の設定(JSON)。
#[no_mangle]
pub extern "system" fn Java_tokyo_aon_pdfr2lrs_Native_defaultsJson<'l>(mut env: JNIEnv<'l>, _c: JClass<'l>) -> jstring {
    let json = serde_json::to_string(&Settings::default()).unwrap_or_default();
    to_jstring(&mut env, &json)
}

/// 言語の一覧と日英の説明(JSON)。
#[no_mangle]
pub extern "system" fn Java_tokyo_aon_pdfr2lrs_Native_languagesJson<'l>(mut env: JNIEnv<'l>, _c: JClass<'l>) -> jstring {
    let langs: Vec<serde_json::Value> = LANGUAGES.iter().map(|(c, ja, en)| serde_json::json!({"code": c, "ja": ja, "en": en})).collect();
    let json = serde_json::json!({
        "languages": langs,
        "language_help_ja": LANGUAGE_HELP_JA, "language_help_en": LANGUAGE_HELP_EN,
        "quality_help_ja": QUALITY_HELP_JA, "quality_help_en": QUALITY_HELP_EN,
        "feature_ja": FEATURE_NOTE_JA, "feature_en": FEATURE_NOTE_EN,
    });
    to_jstring(&mut env, &json.to_string())
}

/// OCR サーバーへの接続テスト。成功なら日英メッセージ、失敗なら RuntimeException。
#[no_mangle]
pub extern "system" fn Java_tokyo_aon_pdfr2lrs_Native_testConnection<'l>(mut env: JNIEnv<'l>, _c: JClass<'l>, url: JString<'l>) -> jstring {
    let Ok(url) = env.get_string(&url) else {
        throw(&mut env, "invalid url");
        return std::ptr::null_mut();
    };
    let url: String = url.into();
    match OcrClient::new(&url).languages() {
        Ok(l) if l.tesseract_available => to_jstring(&mut env, "接続できました。OCR(Tesseract)が使えます。\nConnected. OCR (Tesseract) is available."),
        Ok(_) => {
            throw(&mut env, "サーバーには接続できましたが、Tesseract が使えません。\nConnected, but Tesseract is not available on the server.");
            std::ptr::null_mut()
        }
        Err(e) => {
            throw(&mut env, &e);
            std::ptr::null_mut()
        }
    }
}

/// 画像の高解像度化(バイト列)。`target`: fhd/qhd/4k/5k/8k/16k、`ai`: AI 超解像か補間のみか、`png`: 形式。
/// 結果の大きさ等は message() に入る。失敗・キャンセルは RuntimeException。
pub fn upscale_image_bytes(input: &[u8], target: &str, ai: bool, png: bool) -> Result<Vec<u8>, String> {
    use pdf_r2l_core::upscale::{upscale_bytes, ImageFormat, ImageTarget};
    use pdf_r2l_core::sr::Options as ImageOptions;
    let target = match target {
        "fhd" => ImageTarget::FullHd,
        "qhd" => ImageTarget::Qhd,
        "5k" => ImageTarget::Uhd5k,
        "8k" => ImageTarget::Uhd8k,
        "16k" => ImageTarget::Uhd16k,
        _ => ImageTarget::Uhd4k,
    };
    CANCEL.store(false, Ordering::Relaxed);
    set(&PROGRESS, String::new());
    let opts = ImageOptions { target, ai, format: if png { ImageFormat::Png } else { ImageFormat::Jpeg }, threads: 0 };
    let (bytes, w, h, used_ai) = upscale_bytes(input, opts, &mut |p| set(&PROGRESS, format!("image|{}|{}", (p.fraction * 100.0) as u32, p.stage)), &|| CANCEL.load(Ordering::Relaxed))?;
    set(&MESSAGE, format!("完了: {w}×{h}({})
Done: {w}x{h} ({})", if used_ai { "AI超解像" } else { "補間のみ" }, if used_ai { "AI super-resolution" } else { "interpolation only" }));
    Ok(bytes)
}

/// `external fun upscale(input: ByteArray, target: String, ai: Boolean, png: Boolean): ByteArray`
#[no_mangle]
pub extern "system" fn Java_tokyo_aon_pdfr2lrs_Native_upscale<'l>(mut env: JNIEnv<'l>, _c: JClass<'l>, input: JByteArray<'l>, target: JString<'l>, ai: jni::sys::jboolean, png: jni::sys::jboolean) -> JByteArray<'l> {
    let null = || unsafe { JByteArray::from_raw(std::ptr::null_mut()) };
    let (Ok(bytes), Ok(target)) = (env.convert_byte_array(&input), env.get_string(&target)) else {
        throw(&mut env, "invalid arguments");
        return null();
    };
    let target: String = target.into();
    match upscale_image_bytes(&bytes, &target, ai != 0, png != 0) {
        Ok(out) => env.byte_array_from_slice(&out).unwrap_or_else(|_| null()),
        Err(msg) => {
            throw(&mut env, &msg);
            null()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_settings_and_keep_without_ocr() {
        assert!(convert_bytes(b"x", "not json", "/tmp").is_err());
        let prefs = r#"{"binding":"keep","spread":true,"ocr":{"enabled":false}}"#;
        let err = convert_bytes(b"x", prefs, "/tmp").err().unwrap_or_default();
        assert!(err.contains("Nothing to change"), "{err}");
    }

    #[test]
    fn upscales_a_small_image_and_rejects_garbage() {
        assert!(upscale_image_bytes(b"not an image", "4k", false, true).is_err());
        let mut png = Vec::new();
        let img = image::RgbImage::from_pixel(64, 48, image::Rgb([10, 120, 200]));
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let out = upscale_image_bytes(&png, "fhd", false, true).unwrap();
        assert!(out.len() > png.len());
        assert!(get(&MESSAGE).contains("Done"));
    }
}
