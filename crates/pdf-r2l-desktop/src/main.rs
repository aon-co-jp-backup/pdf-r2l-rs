#![windows_subsystem = "windows"]
//! PDF R2L RS — デスクトップ版(Windows / macOS / Linux)。

use eframe::egui;
use pdf_r2l_core::ocr::languages::{self, FEATURE_NOTE_EN, FEATURE_NOTE_JA, LANGUAGES, LANGUAGE_HELP_EN, LANGUAGE_HELP_JA, QUALITY_HELP_EN, QUALITY_HELP_JA};
use pdf_r2l_core::ocr::{settings as ocr_dirs, OcrClient, Progress, Quality, Settings, Stage, MAX_LANGUAGES};
use pdf_r2l_core::{run, suggest_output_name, Binding, DirectionOptions, Output, Task};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const APP: &str = "pdf-r2l-rs";
const LICENSES: &str = include_str!("../../../LICENSES.txt");

/// 画面で選んだ内容。次回起動時に復元する。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Prefs {
    right_to_left: bool,
    spread: bool,
    overwrite: bool,
    ocr: Settings,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { right_to_left: true, spread: true, overwrite: false, ocr: Settings::default() }
    }
}

fn load_prefs() -> Prefs {
    std::fs::read_to_string(ocr_dirs::config_dir(APP).join("prefs.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save_prefs(p: &Prefs) -> Result<(), String> {
    let dir = ocr_dirs::config_dir(APP);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("prefs.json"), serde_json::to_string_pretty(p).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// 作業スレッドと画面で共有する状態。
#[derive(Default)]
struct Shared {
    running: bool,
    progress: Option<Progress>,
    result: Option<Result<(String, PathBuf), String>>,
    testing: bool,
    test_result: Option<Result<String, String>>,
}

struct App {
    src: Option<PathBuf>,
    prefs: Prefs,
    saved: Prefs,
    language_window: bool,
    license_window: bool,
    language_filter: String,
    shared: Arc<Mutex<Shared>>,
    cancel: Arc<AtomicBool>,
}

const FONT_CANDIDATES: &[&str] = &[
    "C:/Windows/Fonts/YuGothM.ttc",
    "C:/Windows/Fonts/meiryo.ttc",
    "C:/Windows/Fonts/msgothic.ttc",
    "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
    "/System/Library/Fonts/ヒラギノ角ゴシック W4.ttc",
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/Library/Fonts/Arial Unicode.ttf",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/opentype/ipafont-gothic/ipag.ttf",
    "/usr/share/fonts/truetype/fonts-japanese-gothic.ttf",
];

/// 画面用の日本語フォント(OS にあるものを先頭にして、英数字も同じフォントで描く)。
fn setup_fonts(ctx: &egui::Context) {
    for p in FONT_CANDIDATES {
        if let Ok(bytes) = std::fs::read(p) {
            let mut f = egui::FontDefinitions::default();
            f.font_data.insert("jp".into(), egui::FontData::from_owned(bytes));
            for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                f.families.entry(fam).or_default().insert(0, "jp".into());
            }
            ctx.set_fonts(f);
            return;
        }
    }
}

fn stage_text(p: &Progress) -> String {
    match p.stage {
        Stage::Prepare => "言語データを準備中… / Preparing language data… (初回は時間がかかります / the first time takes a while)".to_string(),
        Stage::Ocr => format!("OCR中 / Reading text… {}/{} ページ / pages", p.done, p.total),
        Stage::Compose => "書体を取得して PDF を組み立て中… / Fetching fonts and building the PDF…".to_string(),
    }
}

const GREEN: egui::Color32 = egui::Color32::from_rgb(0, 140, 60);
const RED: egui::Color32 = egui::Color32::from_rgb(200, 40, 40);

impl App {
    fn new(prefs: Prefs) -> Self {
        let ui_debug = std::env::var("PDFR2L_UI_DEBUG").unwrap_or_default();
        App {
            src: None,
            saved: prefs.clone(),
            prefs,
            language_window: ui_debug == "lang",
            license_window: ui_debug == "license",
            language_filter: String::new(),
            shared: Arc::new(Mutex::new(Shared::default())),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    fn binding(&self) -> Binding {
        if self.prefs.right_to_left {
            Binding::RightToLeft
        } else {
            Binding::LeftToRight
        }
    }

    fn running(&self) -> bool {
        self.shared.lock().map(|s| s.running).unwrap_or(false)
    }

    fn set_error(&self, msg: &str) {
        if let Ok(mut s) = self.shared.lock() {
            s.result = Some(Err(msg.to_string()));
        }
    }

    /// `ocr_enabled`: Some(false) でこの回だけ OCR を使わない(設定は変えない)。
    fn start(&mut self, ctx: &egui::Context, ocr_enabled: Option<bool>) {
        let Some(src) = self.src.clone() else { return };
        let output = if self.prefs.overwrite {
            Output::InPlace { backup: true }
        } else {
            let mut dlg = rfd::FileDialog::new().add_filter("PDF", &["pdf"]).set_file_name(suggest_output_name(&src, self.binding()));
            if let Some(dir) = src.parent() {
                dlg = dlg.set_directory(dir);
            }
            let Some(dest) = dlg.save_file() else { return };
            if dest == src {
                self.set_error("元のファイルと同じ場所には保存できません。別の名前で保存するか、「元のファイルを上書き」を選んでください。\nCannot save over the original here. Choose another name, or choose \"Overwrite the original\".");
                return;
            }
            Output::NewFile(dest)
        };
        let mut ocr = self.prefs.ocr.clone();
        if let Some(e) = ocr_enabled {
            ocr.enabled = e;
        }
        let task = Task { source: src, output, direction: DirectionOptions { binding: self.binding(), spread: self.prefs.spread }, ocr: Some(ocr) };
        if let Ok(mut s) = self.shared.lock() {
            s.running = true;
            s.progress = None;
            s.result = None;
        }
        self.cancel.store(false, Ordering::Relaxed);
        let (shared, cancel, ctx) = (Arc::clone(&self.shared), Arc::clone(&self.cancel), ctx.clone());
        std::thread::spawn(move || {
            let cache = ocr_dirs::cache_dir(APP);
            let res = run(
                &task,
                &cache,
                &mut |p| {
                    if let Ok(mut s) = shared.lock() {
                        s.progress = Some(p);
                    }
                    ctx.request_repaint();
                },
                &|| cancel.load(Ordering::Relaxed),
            );
            if let Ok(mut s) = shared.lock() {
                s.running = false;
                s.result = Some(res.map(|o| (o.message, o.destination)));
            }
            ctx.request_repaint();
        });
    }

    fn test_connection(&self, ctx: &egui::Context) {
        {
            let Ok(mut s) = self.shared.lock() else { return };
            if s.testing {
                return;
            }
            s.testing = true;
            s.test_result = None;
        }
        let (shared, ctx, url) = (Arc::clone(&self.shared), ctx.clone(), self.prefs.ocr.server_url.clone());
        std::thread::spawn(move || {
            let res = OcrClient::new(&url).languages().and_then(|l| {
                if l.tesseract_available {
                    Ok("接続できました。OCR(Tesseract)が使えます。\nConnected. OCR (Tesseract) is available.".to_string())
                } else {
                    Err("サーバーには接続できましたが、Tesseract が使えません。\nConnected, but Tesseract is not available on the server.".to_string())
                }
            });
            if let Ok(mut s) = shared.lock() {
                s.testing = false;
                s.test_result = Some(res);
            }
            ctx.request_repaint();
        });
    }

    fn language_summary(&self) -> String {
        self.prefs
            .ocr
            .languages
            .iter()
            .map(|c| languages::name_of(c).map(|(ja, en)| format!("{ja} / {en}")).unwrap_or_else(|| c.clone()))
            .collect::<Vec<_>>()
            .join("、 ")
    }

    fn language_window_ui(&mut self, ctx: &egui::Context) {
        let mut open = self.language_window;
        egui::Window::new("言語を選ぶ / Choose languages").open(&mut open).default_size([640.0, 700.0]).show(ctx, |ui| {
            egui::CollapsingHeader::new("説明 / Help").default_open(true).show(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("lang_help").max_height(150.0).show(ui, |ui| {
                    ui.label(LANGUAGE_HELP_JA);
                    ui.add_space(4.0);
                    ui.label(LANGUAGE_HELP_EN);
                });
            });
            ui.separator();
            let langs = &mut self.prefs.ocr.languages;
            ui.horizontal(|ui| {
                ui.strong(format!("選択中 / Selected: {}/{}", langs.len(), MAX_LANGUAGES));
                if langs.len() >= MAX_LANGUAGES {
                    ui.colored_label(egui::Color32::from_rgb(200, 120, 0), "(上限 / limit reached)");
                }
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button("日本語+英語 / Japanese+English").clicked() {
                    *langs = vec!["jpn".into(), "eng".into()];
                }
                if ui.button("英語のみ / English only").clicked() {
                    *langs = vec!["eng".into()];
                }
                if ui.button("日・英・中(簡)・韓 / JA, EN, ZH-Hans, KO").clicked() {
                    *langs = vec!["jpn".into(), "eng".into(), "chi_sim".into(), "kor".into()];
                }
            });
            ui.horizontal(|ui| {
                ui.label("検索 / Search:");
                ui.text_edit_singleline(&mut self.language_filter);
            });
            ui.separator();
            let filter = self.language_filter.trim().to_lowercase();
            let full = langs.len() >= MAX_LANGUAGES;
            egui::ScrollArea::vertical().id_salt("lang_list").max_height(360.0).show(ui, |ui| {
                for (code, ja, en) in LANGUAGES {
                    if !filter.is_empty() && !format!("{ja} {en} {code}").to_lowercase().contains(&filter) {
                        continue;
                    }
                    let mut on = langs.iter().any(|c| c == code);
                    let r = ui.add_enabled(on || !full, egui::Checkbox::new(&mut on, format!("{ja} / {en}  ({code})")));
                    if r.changed() {
                        languages::toggle(langs, code);
                    }
                }
            });
        });
        self.language_window = open;
    }

    fn ocr_section(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, busy: bool) {
        ui.add_enabled_ui(!busy, |ui| {
            ui.checkbox(&mut self.prefs.ocr.enabled, "OCRして、元の書体に合わせた検索できる文字に置き換える / OCR and replace with searchable text in the matching typeface");
        });
        ui.small(format!("{FEATURE_NOTE_JA}\n{FEATURE_NOTE_EN}"));
        if !self.prefs.ocr.enabled {
            return;
        }
        egui::CollapsingHeader::new("OCR設定 / OCR settings").default_open(true).show(ui, |ui| {
            ui.add_enabled_ui(!busy, |ui| {
                ui.horizontal(|ui| {
                    ui.label("接続先URL / Server URL:");
                    ui.add(egui::TextEdit::singleline(&mut self.prefs.ocr.server_url).desired_width(260.0));
                    if ui.button("接続テスト / Test").clicked() {
                        self.test_connection(ctx);
                    }
                    if ui.small_button("既定 / Default").clicked() {
                        self.prefs.ocr.server_url = Settings::default().server_url;
                    }
                });
                ui.small("既定は、この PC で動いている aruaru-llm です。/ Default: aruaru-llm running on this PC.");
                if let Ok(s) = self.shared.lock() {
                    if s.testing {
                        ui.label("接続を確認中… / Testing…");
                    }
                    match &s.test_result {
                        Some(Ok(m)) => {
                            ui.colored_label(GREEN, m);
                        }
                        Some(Err(e)) => {
                            ui.colored_label(RED, e);
                        }
                        None => {}
                    }
                }
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("言語 / Languages ({}/{MAX_LANGUAGES}):", self.prefs.ocr.languages.len()));
                    if ui.button("言語を選ぶ… / Choose…").clicked() {
                        self.language_window = true;
                    }
                });
                ui.label(self.language_summary());
                ui.label("文字の扱い / Text handling:");
                ui.radio_value(&mut self.prefs.ocr.replace_text, true, "元の書体に合わせた文字に置き換える(かすれた文字も読みやすく) / Replace with text in the matching typeface (faded text becomes readable)");
                ui.radio_value(&mut self.prefs.ocr.replace_text, false, "元の画像はそのまま、検索用の見えない文字だけ追加(誤認識が心配なとき) / Keep the original image; add invisible searchable text only (if you worry about misreads)");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("品質 / Quality:");
                    ui.radio_value(&mut self.prefs.ocr.quality, Quality::Best, "高精度 / Best");
                    ui.radio_value(&mut self.prefs.ocr.quality, Quality::Fast, "高速 / Fast");
                });
                ui.small(format!("{QUALITY_HELP_JA}\n{QUALITY_HELP_EN}"));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("置き換える信頼度のしきい値 / Replace only above confidence:");
                    ui.add(egui::Slider::new(&mut self.prefs.ocr.min_confidence, 0.0..=90.0).step_by(5.0));
                });
                ui.small("この値より自信のない行は、元の画像のまま残し、検索用の見えない文字だけ追加します。/ Lines below this confidence keep the original image and only get invisible searchable text.");
            });
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let busy = self.running();
        if !busy {
            ctx.input(|i| {
                if let Some(f) = i.raw.dropped_files.first().and_then(|f| f.path.clone()) {
                    self.src = Some(f);
                }
            });
        }
        if self.language_window {
            self.language_window_ui(ctx);
        }
        egui::Window::new("ライセンス / License").open(&mut self.license_window).default_size([560.0, 400.0]).show(ctx, |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                ui.add(egui::Label::new(egui::RichText::new(LICENSES.trim_start_matches('\u{feff}')).monospace().size(11.0)).wrap_mode(egui::TextWrapMode::Extend));
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("PDF 綴じ方向変更 / PDF binding direction");
                ui.add_space(6.0);
                ui.label("PDFファイルをドラッグ&ドロップ、または選択: / Drop a PDF here, or choose one:");
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!busy, |ui| {
                        if ui.button("PDFを選択… / Choose PDF…").clicked() {
                            if let Some(p) = rfd::FileDialog::new().add_filter("PDF", &["pdf"]).pick_file() {
                                self.src = Some(p);
                            }
                        }
                    });
                    ui.label(self.src.as_ref().map(|p| p.display().to_string()).unwrap_or("(未選択 / none)".into()));
                });
                ui.add_space(6.0);
                ui.add_enabled_ui(!busy, |ui| {
                    ui.radio_value(&mut self.prefs.right_to_left, true, "右綴じ (R2L) / Right binding");
                    ui.radio_value(&mut self.prefs.right_to_left, false, "左綴じ (L2R) / Left binding");
                    ui.checkbox(&mut self.prefs.spread, "見開き表示にする / Show as two-page spread");
                    ui.small("ビューアーで2ページ並べて表示します(右綴じは奇数ページが右、左綴じは左)。/ Viewers show two pages side by side (odd pages on the right for right binding, on the left for left binding).");
                    ui.add_space(4.0);
                    ui.radio_value(&mut self.prefs.overwrite, false, "別のファイルに保存 / Save as a new file");
                    ui.radio_value(&mut self.prefs.overwrite, true, "元のファイルを上書き(元は .bak に残す) / Overwrite the original (kept as .bak)");
                });
                ui.separator();
                self.ocr_section(ui, ctx, busy);
                ui.separator();
                ui.horizontal(|ui| {
                    let label = if self.prefs.overwrite { "変換して上書き / Convert and overwrite" } else { "変換して保存… / Convert and save…" };
                    if ui.add_enabled(self.src.is_some() && !busy, egui::Button::new(label)).clicked() {
                        self.start(ctx, None);
                    }
                    if busy && ui.button("キャンセル / Cancel").clicked() {
                        self.cancel.store(true, Ordering::Relaxed);
                    }
                });
                let mut offline_fallback = false;
                if let Ok(s) = self.shared.lock() {
                    if s.running {
                        match &s.progress {
                            Some(p) => {
                                let frac = if p.total == 0 { 0.0 } else { p.done as f32 / p.total as f32 };
                                ui.add(egui::ProgressBar::new(frac).text(stage_text(p)));
                            }
                            None => {
                                ui.add(egui::ProgressBar::new(0.0).text("開始中… / Starting…"));
                            }
                        }
                        ui.spinner();
                    }
                    match &s.result {
                        Some(Ok((m, dest))) => {
                            ui.colored_label(GREEN, m);
                            #[cfg(target_os = "windows")]
                            if ui.button("保存先を開く / Show in folder").clicked() {
                                let _ = std::process::Command::new("explorer").arg(format!("/select,{}", dest.display())).spawn();
                            }
                            #[cfg(not(target_os = "windows"))]
                            let _ = dest;
                        }
                        Some(Err(e)) => {
                            ui.colored_label(RED, format!("エラー / Error: {e}"));
                            if e.contains("online environment") && !busy {
                                offline_fallback = true;
                            }
                        }
                        None => {}
                    }
                }
                if offline_fallback && self.src.is_some() && ui.button("OCRを使わずに、綴じ方向だけ変換 / Convert without OCR (direction only)").clicked() {
                    self.start(ctx, Some(false));
                }
                ui.add_space(6.0);
                if ui.button("ライセンス / License").clicked() {
                    self.license_window = true;
                }
            });
        });
        // 画面で選んだ内容が変わったら保存する(次回起動時に復元)。
        if self.prefs != self.saved && save_prefs(&self.prefs).is_ok() {
            self.saved = self.prefs.clone();
        }
    }
}

fn main() -> eframe::Result {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([700.0, 780.0]).with_min_inner_size([520.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        "PDF R2L RS",
        opts,
        Box::new(|cc| {
            setup_fonts(&cc.egui_ctx);
            Ok(Box::new(App::new(load_prefs())))
        }),
    )
}
