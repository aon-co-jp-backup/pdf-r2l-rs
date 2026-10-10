# pdf-r2l-rs

PDFの**綴じ方向(右綴じ R2L / 左綴じ L2R)**を変更し、**見開き表示**にし、スキャンしたページを **OCR して元の書体に合わせた検索できる文字に置き換える**、
Windows / macOS / Linux(・Android)用の GUI アプリです(インストーラー付き)。Rust 製。

A GUI app for Windows / macOS / Linux (and Android) that changes the **binding direction (R2L / L2R)** of PDFs, shows them as a **two-page spread**,
and can **OCR scanned pages and replace the text with searchable text in a matching typeface**. Written in Rust, with installers.

## 機能 / Features

- **綴じ方向**: 右綴じ(R2L)/左綴じ(L2R)/**元と同じ**(変更しない。OCR だけ使いたいとき)。PDF の `/ViewerPreferences /Direction` を設定します。他の表示設定は保持します。
- **見開き表示**: `/PageLayout` を `TwoColumnRight`(右綴じ)/`TwoColumnLeft`(左綴じ)にします(オフにもできます)。
- **保存**: 別のファイルに保存(既定)/元のファイルを上書き(元は `.bak` に残します)。一時ファイル経由で保存するので、途中で失敗しても元のファイルを壊しません。
- **OCR・テキストフォント文字化**(既定でオン、チェックを外すと元のデータのまま):
  - ページを画像にして `aruaru-llm`(`POST /v1/ocr`、Tesseract)で OCR(複数ページをまとめて処理)。
  - 行ごとに書体の系統(ゴシック体/明朝体)・太字・文字色・背景色を推定し、Noto フォント(Sans / Serif、日・簡・繁・韓)のサブセットを PDF に埋め込んで置き換えます。**検索・コピーができる文字**になります。
  - 言語は **1〜12個**まで選べ(日本語・英語・中国語・韓国語・EU 言語・ロシア語・ウクライナ語・ペルシャ語・アラビア語ほか 49 言語)、**選んだ言語のデータだけ**を取得してキャッシュします。
  - 「**元の画像はそのまま、検索用の見えない文字だけ追加**」も選べます(誤認識が心配なとき)。
  - **オンライン環境が必要**です(OCR サーバーへ接続)。ページ画像は処理後すぐ破棄されます。OCR・書体の判定には誤りがあり得ます。

## インストール / Install

Releases から、お使いの OS 用のものを入手します。

| OS | 配布物 |
|---|---|
| Windows | `pdf-r2l-rs-setup.exe`(インストーラー) |
| macOS | `pdf-r2l-rs-*.dmg`(未署名・未公証。初回は右クリック→「開く」) |
| Linux | `pdf-r2l-rs_*.deb`(Debian/Ubuntu 系)または `pdf-r2l-rs-x86_64.tar.gz` |
| Android | `pdf-r2l-rs.apk`(署名済み。Android 8.0 以上。「提供元不明のアプリ」の許可が必要) |

## 画像の高解像度化 / Upscale images (v1.1.0)

**日本語**: 写真・画像(PNG/JPEG)を、**フルHD / QHD(FHDと4Kの間) / 4K / 5K(4Kと8Kの間) / 8K / 16K** の枠(縦横比を保って収める)へ拡大します。
AI超解像(Real-ESRGAN realesr-general-x4v3、CPU)+高品質な補間。AIを使わず補間のみも選べます。使うかどうかは利用者の自由で、元の画像は変更せず別名で保存します。
AI超解像は失われた細部を**推測して補う**もので、実際の細部の復元ではありません。CPU処理のため時間がかかり(Arm のスマホは特に遅い)、8K・16K は大量のメモリとファイルサイズが必要です。
デスクトップ版は複数枚をまとめて処理、Android 版は1枚ずつです。Android 版は実機(moto g53y 5G、縦・横)で画面とAI超解像の動作を確認済み。Arm向けNEONカーネルにより、480×320の画像をフルHDへ拡大するAI処理は約51秒(スカラー実装の337秒から高速化)。大きな写真は数分以上かかります。

**English**: Enlarge photos/images (PNG/JPEG) to Full HD / QHD (between FHD and 4K) / 4K / 5K (between 4K and 8K) / 8K / 16K frames, keeping the aspect ratio, using
CPU AI super-resolution (Real-ESRGAN realesr-general-x4v3) plus high-quality resampling (interpolation-only is selectable). Entirely optional; the original is never modified.
AI super-resolution guesses missing detail rather than restoring it; it is slow on the CPU (especially on Arm phones), and 8K/16K need lots of memory and disk.
The desktop app batches several images; Android handles one at a time. The Android screen and AI path were verified on a real phone (moto g53y 5G, portrait and landscape); with the Arm NEON kernel a 480x320 image to Full HD takes about 51 s (337 s with the scalar code). Large photos take minutes or more.

## AI文脈補完 / AI context completion (v1.2.0, 実験的 / experimental)

**日本語**: OCRで**かすれて読めない・自信が低い1〜2文字**を、**前後の長い文脈から推測して穴埋め**します(長文の書き換えではありません)。既定はオフ(設定でオン)。
- 対象: 信頼度の低い短い単語、文字がまるごと欠けた疑いのある隙間(CJK)、余計に挿入された1文字。
- 方式: 形の似た字の表(日↔曰、0↔O、`.`↔`,`、キリル文字の混入など)と言語モデル自身の予測から候補を作り、**「候補+後ろの文脈」が前の文脈に続く確率(対数尤度)**を言語モデル(Qwen2.5)で採点。元の読みより十分に尤もらしいときだけ、1か所・最大2文字を直します。
- サーバー: `aruaru-llm` の `POST /v1/ocr/fill`(`open-cuda` の Qwen にバッチ処理と候補採点API `score_continuations` / `top_next_tokens` を追加)。言語モデル(`/v1/qwen/install` → `/v1/qwen/select`)が必要で、無いときは何も変えずに進みます。OCRとは別のサーバー(手元のPCなど)も指定できます。
- 実測(Qwen2.5-0.5B、CPU): 「私たちは**人**毎朝」→削除、「10.000円」→「10,000円」、「署名と押印」「今日」の取り違え防止などを確認。**まるごと消えた文字は文脈からの推測にとどまり**(例: 「今□は」を「今朝は」と補った。正解は「今日」)、誤ることがあります。1か所あたり数秒〜(CPU)。結果の文章は必ず確認してください。

**English**: Fills in **one or two faded / unreadable characters** that OCR missed or read with low confidence, **guessing from the long surrounding context** (it never rewrites long text). Off by default.
- Targets: short low-confidence words, suspicious gaps where a CJK character is missing, and stray single characters.
- Method: candidates come from a table of look-alike characters (日↔曰, 0↔O, `.`↔`,`, stray Cyrillic letters, ...) and from the language model's own predictions; a language model (Qwen2.5) scores **the log-likelihood of "candidate + following text" given the preceding text**, and a change (one spot, at most two characters) is made only when clearly more likely than the original reading.
- Server: `POST /v1/ocr/fill` in `aruaru-llm` (with batched prefill and candidate scoring APIs `score_continuations` / `top_next_tokens` added to the Qwen engine of `open-cuda`). It needs a language model installed (`/v1/qwen/install` then `/v1/qwen/select`); without one nothing is changed. A different server (e.g. your own PC) can be used for this step.
- Measured (Qwen2.5-0.5B on CPU): removed a stray "人", fixed "10.000" to "10,000", etc. **A fully erased character is only a guess from context** (e.g. "今□は" was filled as "今朝は"; the truth was "今日") and can be wrong. Several seconds per spot on a CPU. Always check the result.

## OCR を使うには / Using OCR

既定の接続先は、同じ PC で動いている `aruaru-llm`(`http://127.0.0.1:4600`)です。アプリの「OCR設定」で別の URL(LAN 内の別の端末など)に変更し、
「接続テスト」で確認できます。サーバーには Tesseract が必要です(言語データは `aruaru-llm` が必要な言語だけ自動で取得します)。
OCR に接続できないときは、「OCRを使わずに、綴じ方向だけ変換」で続けられます。

## 開発 / Build

```
cargo test -p pdf-r2l-core
cargo build --release            # target/release/pdf-r2l-rs (GUI), pdf-r2l-cli (command line)
"C:\Program Files (x86)\Inno Setup 6\ISCC.exe" installer\pdf-r2l-rs.iss   # Windows installer
```

コマンドライン版: `pdf-r2l-cli 入力.pdf 出力.pdf [--ltr|--keep] [--no-spread] [--ocr URL] [--lang jpn+eng] [--fast] [--search-only]`

依存を変えたら `scripts/gen-third-party.ps1 > licenses/third-party.txt` の後に `sh scripts/build-licenses.sh` で `LICENSES.txt` を再生成してください。

## LICENSE

独自実装です。綴じ方向の書き換えは PDF の仕様書(ISO 32000-1)だけを根拠に書いています。
**MIT または Apache-2.0**(どちらかを選べます)。第三者ライセンスは [LICENSES.txt](LICENSES.txt)(英日併記)。

## 開発中(未リリース)の記録: 2026-10-10 開発停止時点(OCR かすれ補正・AI 文脈補完) / Work in progress (unreleased): status at stop

### 日本語

**到達点(実装済み・単体試験済み)**
- **コントラスト自動補正**(`open-runo-pdf-ocr::enhance::normalize_contrast`): 薄い文書は「白紙」と判定されて OCR にも送られなかった。濃淡を引き伸ばして OCR へ送るようにした(試験用のかすれ文書で 0% → 48%)。常にオン、十分に濃いページには触れない。
- **かすれ補正**(設定 `reocr`、既定オフ): 自信の低い行を切り出し → ノイズ除去 → AI 超解像(`open-runo-sr`)または補間で 3 倍 → 再 OCR。2 通りの読み方のうち信頼度が高いほうを採用(同じ試験文書で 89%)。AI 超解像は、ノイズを先に除かないと字が崩れる(ブロック状になる)ことを実測で確認した。
- **AI 文脈補完**(設定 `fill`、既定オフ・実験的): 言語モデル(Qwen2.5)が各行を走査して不自然な 1〜4 文字を検出(`POST /v1/ocr/scan`、トークンごとの対数確率)→ 候補を採点(`POST /v1/ocr/fill`)→ 置換。候補の出どころは、形の似た字の表、字形の照合、検索結果から採掘した文字列(`aruaru-search`)、言語モデルの予測。連続した複数文字の置換は、複数の単語を 1 つに結合して反映する。
- **字形の照合**(`open-runo-glyph`、新設の共有クレート): かすれた升目と、候補の字形(明朝・ゴシック)を正規化相互相関で比較。升目の中で字形をずらす全位置を 1 回の**行列積**にして、サーバー(`POST /v1/ocr/glyph-rank`)では `open-cuda` の `sgemm`(CPU は `open-cpu`)で実行する。頻出字約 9,200 字形 × 289 位置で **1 升あたり約 14 ms**(参照実装は約 19 ms。字形の行列の構築は約 19 ms)。アプリ内の CPU 参照実装にも自動で切り替わる。
- **`open-cuda` の Qwen**: バッチ prefill、`score_continuations`(候補の対数尤度)、`top_next_tokens`、`token_logprobs` を追加(1 トークンずつの結果と数値一致を試験で確認)。

**測定結果(正直な記録)**: 合成の「かすれ文書」(小さく・薄く・ぼけ・ノイズ)での文字正解率。コントラスト補正のみ 48%、+ かすれ補正 89%、+ AI 文脈補完(初期版)86〜90%、+ 連続文字・検索を加えた版は **87%(880 秒)と逆効果**だった。原因: 文脈そのものに誤字が多いページでは、言語モデルの採点が後ろの誤字に引きずられる/自由な候補が的外れになる。
その後、精度優先に作り直した(字形が元の読みより明らかに悪い候補は採らない、言語モデルの自由な候補は字形があるとき使わない、検索は上位 2 候補のみ、字形はサーバーの sgemm へ)が、**作り直し後の通し測定は未実施**(停止のため)。**AI 文脈補完は引き続き実験的・既定オフ**とする。

**未了・次にやること**
1. 作り直し後の通し測定(`faded2.pdf` / `faded.pdf` で、補完なし・字形のみ・字形+検索を比較)。劣化が少ないページで悪化しないこと(精度重視)を確認する。
2. 重く劣化したページ向けに、修正を 1 か所ずつ確定して文脈を更新しながら繰り返す(反復)方式、または字形の発生確率 + 言語モデルの確率を合わせるビーム探索(ノイジーチャネル)。
3. 升目の位置の精度(行の枠からの按分では位置がずれる)。実測: 位置が既知なら 90 字種中の正解を 70〜91% で 1 位にできるが、ずれると下がる。
4. 補完 1 か所あたりの時間(言語モデル 1.5B で 10〜20 秒、検索を併用するとさらに遅い)。モデルの並列化・候補数の削減。
5. 日本語向けのより大きいモデル(Qwen2.5-3B は分割 safetensors のため `QwenModel::load` の対応が要る)。
6. Android 版のかすれ補正の速度確認(実機で AI 超解像は 480×320 → FHD で約 51 秒)。

**サーバー(aruaru-llm)の追加 API**: `POST /v1/ocr/scan`(不自然な箇所の検出)、`POST /v1/ocr/propose`(1 文字候補の提案)、`POST /v1/ocr/fill`(候補の採点・置換。`return_all` / `use_web` / `lm_candidates`)、`GET /v1/ocr/charset?n=`(頻出の漢字・かな)、`POST /v1/ocr/glyph-rank`(字形の照合、sgemm)。いずれも言語モデル(`/v1/qwen/install` → `/v1/qwen/select`)が必要(無いとき 503。`glyph-rank` は文書内の字だけで動く)。検索は `ARUARU_LLM_SEARCH_URL`(既定 `http://127.0.0.1:4610` = `aruaru-search`、VPS 内からは回数制限なし。ローカルで試すときは SSH 中継 `-L 14610:127.0.0.1:4610`)。

**試験のやり方**: 試験用画像は Python(Pillow)で作った小さく薄くぼかしたかすれ文書(正解が分かる)。`RPoem/crates/open-runo-pdf-ocr/examples/e2e.rs`(`PDFOCR_REOCR=1 PDFOCR_DEBUG=1 cargo run --release -p open-runo-pdf-ocr --example e2e -- in.pdf out.pdf <OCRサーバーURL> jpn+eng best 8 <補完サーバーURL>`)で OCR → 補完 → PDF を作り、`pdftotext` の結果を正解と比べる。`examples/vis_probe.rs` は字形の照合だけを、位置が既知の文書で測る(`JITTER=0.3` で位置ずれを与える)。OCR は VPS の `aruaru-llm`(SSH 中継 `-L 14600:127.0.0.1:4600`)、補完は手元の `aruaru-llm`(Qwen2.5-1.5B)を使った。Windows の bash から日本語を `curl` に直接渡すと文字化けするので、Python から UTF-8 で送ること。

### English

**Where things stand (implemented, unit-tested)**
- **Automatic contrast correction** (`open-runo-pdf-ocr::enhance::normalize_contrast`): faint documents used to be classified as "blank" and never sent to OCR. The page is now stretched before OCR (0% → 48% on the synthetic faded test document). Always on; pages with enough contrast are left untouched.
- **Faded-text rescue** (setting `reocr`, off by default): crop low-confidence lines → denoise → 3x enlargement with AI super-resolution (`open-runo-sr`) or interpolation → OCR again; the more confident of two readings is adopted (89% on the same test document). Measured: AI super-resolution breaks glyphs into blocks unless the noise is removed first.
- **AI context completion** (setting `fill`, off by default, experimental): a language model (Qwen2.5) scans every line and flags unnatural 1–4 character spans (`POST /v1/ocr/scan`, per-token log-probabilities), candidates are scored (`POST /v1/ocr/fill`) and replaced. Candidates come from a look-alike table, glyph matching, strings mined from search results (`aruaru-search`), and the model's own predictions. A replacement spanning several words merges them into one.
- **Glyph matching** (`open-runo-glyph`, a new shared crate): a faded cell is compared with candidate glyphs (serif and sans) by normalized cross-correlation. All positions of the glyph inside the cell become **one matrix product**, executed by `open-cuda`'s `sgemm` (CPU: `open-cpu`) in the server (`POST /v1/ocr/glyph-rank`): about **14 ms per cell** for ~9,200 glyphs × 289 positions (reference implementation ~19 ms; building the glyph matrix ~19 ms). The app falls back to the CPU reference automatically.
- **Qwen in `open-cuda`**: batched prefill, `score_continuations` (candidate log-likelihood), `top_next_tokens`, `token_logprobs` (verified numerically identical to token-by-token decoding).

**Measurements (honest record)**: character accuracy on a synthetic faded document (small, faint, blurred, noisy). Contrast correction only 48%; + faded-text rescue 89%; + AI context completion (first version) 86–90%; the version with multi-character spans and web search was **87% in 880 s, i.e. worse**. Cause: on pages where the context itself is full of errors, the model's scoring is dragged by the neighbouring errors and free-form candidates are off target.
It was then reworked for precision (a candidate that is visually clearly worse than the original reading is never taken; free model candidates are not used when glyph evidence exists; search only for the top 2 candidates; glyph matching moved to the server's sgemm), but **the end-to-end measurement after the rework has not been done** (we stopped). **AI context completion stays experimental and off by default.**

**Open items / next steps**
1. Re-measure end to end after the rework (`faded2.pdf` / `faded.pdf`: no completion vs glyph-only vs glyph+search); confirm it does not hurt mildly degraded pages (precision first).
2. For heavily degraded pages: confirm one fix at a time and update the context (iteration), or a beam search combining glyph likelihood with language-model probability (noisy channel).
3. Cell localization accuracy (apportioning the line box drifts). Measured: with known positions the true character ranks first 70–91% of the time among 90 glyphs; it drops when positions drift.
4. Time per spot (10–20 s with the 1.5B model, slower with search): parallelism and fewer candidates.
5. A larger Japanese-capable model (Qwen2.5-3B is sharded safetensors, which `QwenModel::load` does not support yet).
6. Check the faded-text rescue speed on Android (on a real phone AI super-resolution takes ~51 s for 480×320 → FHD).

**New server APIs (aruaru-llm)**: `POST /v1/ocr/scan` (detect unnatural spans), `POST /v1/ocr/propose` (single-character candidates), `POST /v1/ocr/fill` (score and replace; `return_all` / `use_web` / `lm_candidates`), `GET /v1/ocr/charset?n=` (frequent kanji/kana), `POST /v1/ocr/glyph-rank` (glyph matching via sgemm). All need a language model (`/v1/qwen/install` then `/v1/qwen/select`; 503 without one — `glyph-rank` works with the characters found in the document alone). Search goes through `ARUARU_LLM_SEARCH_URL` (default `http://127.0.0.1:4610` = `aruaru-search`, unlimited from inside the VPS; for local tests use an SSH forward `-L 14610:127.0.0.1:4610`).

**How it was tested**: the test images are small, faint, blurred documents made with Python (Pillow) so the ground truth is known. `RPoem/crates/open-runo-pdf-ocr/examples/e2e.rs` (`PDFOCR_REOCR=1 PDFOCR_DEBUG=1 cargo run --release -p open-runo-pdf-ocr --example e2e -- in.pdf out.pdf <OCR server URL> jpn+eng best 8 <completion server URL>`) runs OCR → completion → PDF, and the `pdftotext` result is compared with the truth. `examples/vis_probe.rs` measures glyph matching alone on a document with known positions (`JITTER=0.3` adds position drift). OCR used the VPS `aruaru-llm` (SSH forward `-L 14600:127.0.0.1:4600`); completion used a local `aruaru-llm` (Qwen2.5-1.5B). Passing Japanese text to `curl` directly from Windows bash is garbled — send it from Python as UTF-8.
