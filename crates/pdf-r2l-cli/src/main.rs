//! コマンドライン版。バッチ処理や自動試験に使う。
//!
//! 使い方: `pdf-r2l-cli 入力.pdf 出力.pdf [--ltr|--keep] [--no-spread] [--ocr URL] [--lang jpn+eng] [--fast] [--min-conf 70]`
//! `--in-place` を付けると、出力先を指定せず元のファイルを上書きする(元は `.bak` に残す)。

use pdf_r2l_core::ocr::{Quality, Settings};
use pdf_r2l_core::{run, Binding, DirectionOptions, Output, Task};
use std::path::PathBuf;

fn usage() -> ! {
    eprintln!("usage: pdf-r2l-cli IN.pdf OUT.pdf [--ltr|--keep] [--no-spread] [--ocr URL] [--lang jpn+eng] [--fast] [--search-only] [--min-conf N]\n       pdf-r2l-cli IN.pdf --in-place [options]");
    std::process::exit(2);
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        usage();
    }
    let mut take_flag = |name: &str| -> bool {
        match args.iter().position(|a| a == name) {
            Some(i) => {
                args.remove(i);
                true
            }
            None => false,
        }
    };
    let ltr = take_flag("--ltr");
    let keep = take_flag("--keep");
    let no_spread = take_flag("--no-spread");
    let fast = take_flag("--fast");
    let search_only = take_flag("--search-only");
    let in_place = take_flag("--in-place");
    let mut take_value = |name: &str| -> Option<String> {
        let i = args.iter().position(|a| a == name)?;
        args.remove(i);
        (i < args.len()).then(|| args.remove(i))
    };
    let ocr_url = take_value("--ocr");
    let lang = take_value("--lang");
    let min_conf = take_value("--min-conf");
    let src = PathBuf::from(args.first().cloned().unwrap_or_else(|| usage()));
    let output = if in_place {
        Output::InPlace { backup: true }
    } else {
        Output::NewFile(PathBuf::from(args.get(1).cloned().unwrap_or_else(|| usage())))
    };
    let ocr = ocr_url.map(|url| {
        let mut s = Settings { server_url: url, ..Settings::default() };
        if let Some(l) = &lang {
            s.languages = l.split('+').map(|x| x.to_string()).collect();
        }
        if fast {
            s.quality = Quality::Fast;
        }
        s.replace_text = !search_only;
        if let Some(c) = min_conf.and_then(|c| c.parse().ok()) {
            s.min_confidence = c;
        }
        s
    });
    let task = Task {
        source: src,
        output,
        direction: DirectionOptions { binding: if keep { Binding::Keep } else if ltr { Binding::LeftToRight } else { Binding::RightToLeft }, spread: !no_spread },
        ocr,
    };
    let cache = pdf_r2l_core::ocr::settings::cache_dir("pdf-r2l-rs");
    let t = std::time::Instant::now();
    let mut last = String::new();
    let result = run(
        &task,
        &cache,
        &mut |p| {
            let s = format!("{:?} {}/{}", p.stage, p.done, p.total);
            if s != last {
                eprintln!("[{:.1}s] {s}", t.elapsed().as_secs_f32());
                last = s;
            }
        },
        &|| false,
    );
    match result {
        Ok(o) => println!("{}", o.message),
        Err(e) => {
            eprintln!("ERROR: {e}");
            std::process::exit(1);
        }
    }
}
