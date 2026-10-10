#![windows_subsystem = "windows"]
//! PDF R2L RS — デスクトップ版(Windows / macOS / Linux)。

use eframe::egui;
use pdf_r2l_core::ocr::languages::{self, FEATURE_NOTE_EN, FEATURE_NOTE_JA, LANGUAGES, LANGUAGE_HELP_EN, LANGUAGE_HELP_JA, QUALITY_HELP_EN, QUALITY_HELP_JA};
use pdf_r2l_core::ocr::{settings as ocr_dirs, OcrClient, Progress, Quality, Settings, Stage, MAX_LANGUAGES};
use pdf_r2l_core::sr::{output_size, Format as ImgFormat, Options as SrOptions, Target};
use pdf_r2l_core::{image_dimensions, run, suggest_output_name, summarize_images, upscale_files, Binding, DirectionOptions, Output, Task};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const APP: &str = "pdf-r2l-rs";
const LICENSES: &str = include_str!("../../../LICENSES.txt");

/// 綴じ方向の選択(右綴じ / 左綴じ / 元と同じ)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum BindingPref {
    RightToLeft,
    LeftToRight,
    Keep,
}

/// 画面で選んだ内容。次回起動時に復元する。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Prefs {
    binding: BindingPref,
    spread: bool,
    overwrite: bool,
    ocr: Settings,
    /// 「画像の高解像度化」タブを開いているか。
    tab_images: bool,
    /// 目標の解像度(`Target::ALL` の添字)。
    img_target: usize,
    /// AI 超解像を使うか(偽なら高品質な補間のみ)。
    img_ai: bool,
    /// JPEG で保存するか(偽なら PNG)。
    img_jpeg: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { binding: BindingPref::RightToLeft, spread: true, overwrite: false, ocr: Settings::default(), tab_images: false, img_target: 2, img_ai: true, img_jpeg: false }
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
    /// 画像の高解像度化の進捗(0〜1, 説明)。
    img_progress: Option<(f32, String)>,
    testing: bool,
    test_result: Option<Result<String, String>>,
}

struct App {
    src: Option<PathBuf>,
    images: Vec<PathBuf>,
    prefs: Prefs,
    saved: Prefs,
    language_window: bool,
    license_window: bool,
    language_filter: String,
    shared: Arc<Mutex<Shared>>,
    cancel: Arc<AtomicBool>,
}

/// 画像ファイル(PNG・JPEG)か。
fn is_image(p: &std::path::Path) -> bool {
    p.extension().is_some_and(|e| ["png", "jpg", "jpeg"].iter().any(|x| e.eq_ignore_ascii_case(x)))
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
        // 起動引数の PDF(「このアプリで開く」・ドロップ・コマンドライン)を最初から選んでおく。
        let initial = std::env::args_os().skip(1).map(PathBuf::from).find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) && p.is_file());
        let images: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).filter(|p| is_image(p) && p.is_file()).collect();
        let mut prefs = prefs;
        if initial.is_none() && !images.is_empty() {
            prefs.tab_images = true;
        }
        App {
            src: initial,
            images,
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
        match self.prefs.binding {
            BindingPref::RightToLeft => Binding::RightToLeft,
            BindingPref::LeftToRight => Binding::LeftToRight,
            BindingPref::Keep => Binding::Keep,
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

    /// 画像の高解像度化を、別スレッドで始める。保存先は、元の画像と同じフォルダ(`名前-4k.png` のように印を付ける)。
    fn start_images(&mut self, ctx: &egui::Context) {
        if self.images.is_empty() {
            return;
        }
        let target = Target::ALL[self.prefs.img_target.min(Target::ALL.len() - 1)];
        let opts = SrOptions { target, ai: self.prefs.img_ai, format: if self.prefs.img_jpeg { ImgFormat::Jpeg } else { ImgFormat::Png }, threads: 0 };
        let inputs = self.images.clone();
        let out_dir = inputs[0].parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
        if let Ok(mut s) = self.shared.lock() {
            s.running = true;
            s.img_progress = Some((0.0, "開始中… / Starting…".to_string()));
            s.result = None;
        }
        self.cancel.store(false, Ordering::Relaxed);
        let (shared, cancel, ctx) = (Arc::clone(&self.shared), Arc::clone(&self.cancel), ctx.clone());
        std::thread::spawn(move || {
            let res = upscale_files(
                &inputs,
                &out_dir,
                opts,
                &mut |f, text| {
                    if let Ok(mut s) = shared.lock() {
                        s.img_progress = Some((f, text));
                    }
                    ctx.request_repaint();
                },
                &|| cancel.load(Ordering::Relaxed),
            );
            if let Ok(mut s) = shared.lock() {
                s.running = false;
                s.img_progress = None;
                s.result = Some(res.map(|o| {
                    let dest = o.first().map(|x| x.destination.clone()).unwrap_or_default();
                    (summarize_images(&o), dest)
                }));
            }
            ctx.request_repaint();
        });
    }

    /// 「画像の高解像度化」タブの中身。
    fn image_section(&mut self, ui: &mut egui::Ui, busy: bool) {
        ui.label("画像(PNG・JPEG)をドラッグ&ドロップ、または選択(複数可): / Drop images (PNG/JPEG) here, or choose some:");
        ui.add_enabled_ui(!busy, |ui| {
            if ui.button("画像を選択… / Choose images…").clicked() {
                if let Some(p) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg"]).pick_files() {
                    self.images = p;
                }
            }
        });
        if self.images.is_empty() {
            ui.label("(未選択 / none)");
        } else {
            egui::ScrollArea::vertical().id_salt("img_list").max_height(80.0).show(ui, |ui| {
                for p in &self.images {
                    ui.label(p.display().to_string());
                }
            });
        }
        let first_dims = self.images.first().and_then(|p| image_dimensions(p).ok());
        if let Some((w, h)) = first_dims {
            ui.small(format!("1枚目の元の大きさ / First image: {w}×{h}"));
        }
        ui.add_space(6.0);
        ui.label("目標の解像度(縦横比を保って、この枠に収めます) / Target resolution (fits inside this frame, keeping the aspect ratio):");
        ui.add_enabled_ui(!busy, |ui| {
            for (i, t) in Target::ALL.iter().enumerate() {
                let (ja, en) = t.label();
                let size = first_dims.map(|(w, h)| output_size(w, h, *t)).map(|(w, h)| format!(" → {w}×{h}")).unwrap_or_default();
                ui.radio_value(&mut self.prefs.img_target, i, format!("{ja} / {en}{size}"));
            }
            ui.add_space(4.0);
            ui.radio_value(&mut self.prefs.img_ai, true, "AI超解像で細部を補って拡大(おすすめ・時間がかかる) / Enlarge with AI super-resolution (recommended, slow)");
            ui.radio_value(&mut self.prefs.img_ai, false, "高品質な補間のみで拡大(速い・細部は補わない) / High-quality interpolation only (fast, no added detail)");
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("保存形式 / Format:");
                ui.radio_value(&mut self.prefs.img_jpeg, false, "PNG(可逆・大きい) / PNG (lossless, large)");
                ui.radio_value(&mut self.prefs.img_jpeg, true, "JPEG(小さい) / JPEG (small)");
            });
        });
        ui.add_space(6.0);
        ui.small(
            "元の画像は変更せず、同じフォルダに「名前-4k.png」のように保存します。\nOriginals are untouched; results are saved next to them, e.g. \"name-4k.png\".\n\
             AI超解像は失われた細部を推測して補うもので、実際にあった細部の復元ではありません。CPUで処理するため時間がかかり(200万画素で数十秒〜数分)、8K・16Kはメモリとファイルサイズも大きくなります。Arm の CPU では特に遅くなります。\n\
             AI super-resolution guesses missing detail; it is not a restoration of real detail. It runs on the CPU and is slow (tens of seconds to minutes for 2 megapixels); 8K and 16K need a lot of memory and disk space, and Arm CPUs are much slower.",
        );
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
                ui.add_space(6.0);
                ui.separator();
                ui.checkbox(&mut self.prefs.ocr.fill, "AI文脈補完(かすれて読めない1〜2文字を、前後の文脈から推測して穴埋め) / AI context completion (guess one or two faded characters from the surrounding text)");
                ui.small("信頼度の低い短い単語・欠けた文字・余計な1文字だけを対象に、言語モデルが候補を文脈つきで採点し、十分に尤もらしいときだけ直します。長文は書き換えません。推測なので誤ることがあります(特に、まるごと消えた文字)。サーバーに言語モデル(例: Qwen2.5)の導入が必要で、CPUでは1か所あたり数秒かかります。\nOnly low-confidence short words, missing characters and stray single characters are considered; a language model scores candidates in context and a change is made only when clearly more likely. Long text is never rewritten. It is a guess and can be wrong (especially for a fully erased character). The server needs a language model (e.g. Qwen2.5) and each spot takes seconds on a CPU.");
                if self.prefs.ocr.fill {
                    ui.horizontal(|ui| {
                        ui.label("補完サーバーURL(空ならOCRと同じ) / Completion server URL (blank = same as OCR):");
                        ui.add(egui::TextEdit::singleline(&mut self.prefs.ocr.fill_server_url).desired_width(260.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("補完する最大箇所数 / Max spots:");
                        ui.add(egui::Slider::new(&mut self.prefs.ocr.fill_max_spots, 5..=200).step_by(5.0));
                    });
                }
            });
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let busy = self.running();
        if !busy {
            ctx.input(|i| {
                let paths: Vec<PathBuf> = i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect();
                let pics: Vec<PathBuf> = paths.iter().filter(|p| is_image(p)).cloned().collect();
                if let Some(pdf) = paths.iter().find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))) {
                    self.src = Some(pdf.clone());
                    self.prefs.tab_images = false;
                } else if !pics.is_empty() {
                    self.images = pics;
                    self.prefs.tab_images = true;
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
        // 変換ボタン・進捗・結果は、設定が長くスクロールしても常に見えるよう、画面下に固定する。
        egui::TopBottomPanel::bottom("actions").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if self.prefs.tab_images {
                    if ui.add_enabled(!self.images.is_empty() && !busy, egui::Button::new("画像を高解像度化 / Upscale images")).clicked() {
                        self.start_images(ctx);
                    }
                } else {
                    let label = if self.prefs.overwrite { "変換して上書き / Convert and overwrite" } else { "変換して保存… / Convert and save…" };
                    if ui.add_enabled(self.src.is_some() && !busy, egui::Button::new(label)).clicked() {
                        self.start(ctx, None);
                    }
                }
                if busy && ui.button("キャンセル / Cancel").clicked() {
                    self.cancel.store(true, Ordering::Relaxed);
                }
                if ui.button("ライセンス / License").clicked() {
                    self.license_window = true;
                }
            });
            let mut offline_fallback = false;
            if let Ok(s) = self.shared.lock() {
                if let (true, Some((f, text))) = (s.running, &s.img_progress) {
                    ui.add(egui::ProgressBar::new(*f).text(text.clone()));
                } else if s.running {
                    match &s.progress {
                        Some(p) => {
                            let frac = if p.total == 0 { 0.0 } else { p.done as f32 / p.total as f32 };
                            ui.add(egui::ProgressBar::new(frac).text(stage_text(p)));
                        }
                        None => {
                            ui.add(egui::ProgressBar::new(0.0).text("開始中… / Starting…"));
                        }
                    }
                }
                egui::ScrollArea::vertical().max_height(150.0).id_salt("result").show(ui, |ui| match &s.result {
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
                });
            }
            if offline_fallback && self.src.is_some() && ui.button("OCRを使わずに、綴じ方向だけ変換 / Convert without OCR (direction only)").clicked() {
                self.start(ctx, Some(false));
            }
            ui.add_space(4.0);
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("PDF R2L RS");
                ui.add_enabled_ui(!busy, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.prefs.tab_images, false, "PDF 綴じ方向・OCR / PDF binding & OCR");
                        ui.selectable_value(&mut self.prefs.tab_images, true, "画像の高解像度化 / Upscale images");
                    });
                });
                ui.separator();
                if self.prefs.tab_images {
                    self.image_section(ui, busy);
                    return;
                }
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
                    ui.radio_value(&mut self.prefs.binding, BindingPref::RightToLeft, "右綴じ (R2L) / Right binding");
                    ui.radio_value(&mut self.prefs.binding, BindingPref::LeftToRight, "左綴じ (L2R) / Left binding");
                    ui.radio_value(&mut self.prefs.binding, BindingPref::Keep, "元と同じ(綴じ方向・見開きを変更しない) / Same as the original (no change)");
                    ui.add_enabled(self.prefs.binding != BindingPref::Keep, egui::Checkbox::new(&mut self.prefs.spread, "見開き表示にする / Show as two-page spread"));
                    ui.small("ビューアーで2ページ並べて表示します(右綴じは奇数ページが右、左綴じは左)。/ Viewers show two pages side by side (odd pages on the right for right binding, on the left for left binding).");
                    ui.add_space(4.0);
                    ui.radio_value(&mut self.prefs.overwrite, false, "別のファイルに保存 / Save as a new file");
                    ui.radio_value(&mut self.prefs.overwrite, true, "元のファイルを上書き(元は .bak に残す) / Overwrite the original (kept as .bak)");
                });
                ui.separator();
                self.ocr_section(ui, ctx, busy);
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
