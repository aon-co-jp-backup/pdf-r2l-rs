//! PDF の綴じ方向(右綴じ/左綴じ)・見開き表示と、OCR によるテキストフォント文字化をまとめた中核ライブラリ。
//! デスクトップ版(Windows / macOS / Linux)と Android 版が共通で使う。
//!
//! PDF の書き換えは、PDF の仕様書(ISO 32000-1)だけを根拠に書いた([`direction`])。
//! OCR・ページ描画・書体判定・フォント埋め込みは、共有クレート `open-runo-pdf-ocr`(RPoem)を使う。

pub mod direction;
pub mod task;
pub mod upscale;

pub use direction::{apply, read_binding, Binding, DirectionOptions};
pub use open_runo_pdf_ocr as ocr;
pub use task::{run, suggest_output_name, summarize, transform, Outcome, Output, Task};
pub use open_runo_sr as sr;
pub use upscale::{image_dimensions, suggest_image_name, summarize_images, upscale_bytes, upscale_files, ImageOutcome};
