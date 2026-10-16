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

## v1.3.0(2026-10-10): かすれ補正・字形照合・AI文脈補完の拡張 / v1.3.0: faded-text rescue, glyph matching, extended AI context completion

### 日本語
**v1.3.0 の内容(既定の動作)**
- **コントラスト自動補正(常にオン)**: 薄い・かすれた文書は、背景とインクの差が小さいために「白紙」と判定され、OCR にも送られなかった。濃淡を引き伸ばしてから OCR へ送る(十分に濃いページには触れない)。合成のかすれ文書で正解率 **0% → 約 48%**。
- **かすれ補正(設定 `reocr`、既定オフ)**: 自信の低い行を切り出し → ノイズ除去 → AI 超解像(`open-runo-sr`)と補間の 2 通りで 3 倍に拡大 → 再 OCR し、信頼度の高いほうを採用。同じ文書で **約 48% → 89%**。AI 超解像は、ノイズを先に除かないと字がブロック状に崩れることを実測で確認したため、必ず軽いぼかしの後に行う。
- **AI 文脈補完(設定 `fill`、既定オフ・実験的)**: 言語モデル(Qwen2.5)が各行を走査して不自然な 1〜4 文字を検出 → 形の似た字・字形の照合・検索結果(`aruaru-search`)・言語モデルから候補を作り、文脈つきで採点して置換。連続した複数文字の置換は、複数の単語を 1 つに結合して反映する。字形の証拠が強いときは、字形が元の読みより明らかに悪い候補は採らない(精度優先)。
- **字形の照合**(新設の共有クレート `open-runo-glyph`): かすれた升目と候補の字形(明朝・ゴシック)を正規化相互相関で比較。升目の中で字形をずらす全位置を **1 回の行列積**にして、サーバーの `POST /v1/ocr/glyph-rank` では `open-cuda` の `sgemm`(CPU は `open-cpu` の AVX2)で実行する(約 9,200 字形 × 289 位置で **1 升約 14 ms**)。サーバーが対応していなければ、アプリ内の CPU 参照実装に自動で切り替わる(スマホでフォントを取得せずに済むよう、サーバー優先)。
- **`open-cuda` の Qwen**: バッチ prefill、`score_continuations`(候補の対数尤度)、`top_next_tokens`、`token_logprobs`、語彙ロジットの `sgemm` 化(1 トークンずつの結果・スレッド版と数値一致を試験で確認)。
- 画面: 「かすれ補正」「AI 文脈補完」(字形・検索・サーバー URL・最大箇所数)の設定を、デスクトップと Android に追加。結果表示に「読み直した行数」「補完した箇所数」を追加。

**測定結果(正直な記録)**: 合成の「かすれ文書」(小さく・薄く・ぼけ・ノイズ。正解が分かる 159 文字)。

| 条件 | 文字の正解率 | 時間 |
|---|---|---|
| 以前(白紙と判定され OCR されず) | 0% | – |
| コントラスト自動補正のみ | 約 48% | 数秒 |
| + かすれ補正 | 89.3% | 約 20 秒 |
| + AI 文脈補完(精度優先版・Qwen2.5-1.5B・CPU) | 89.3% | 約 220 秒 |
| 軽いかすれ文書(補正前から 98.7%)に補完を適用 | 98.7%(悪化なし) | 約 100 秒 |

AI 文脈補完は、**これらの試験では害はないが上積みも無く、遅い**(1 か所あたり 10〜20 秒。計算量は言語モデルの大きさ × 候補数 × トークン数で決まる)。途中の版(連続文字・検索を無制限に使う版)は 87%(880 秒)と**悪化**したため、候補数・後ろ文脈・検索の対象を絞って精度優先に作り直した。完全に消えた字は文脈からの推測にとどまり、誤ることがある(例:「今□は」を「今朝は」)。よって**既定オフ・実験的**のままとする。

**サーバー(aruaru-llm)の追加 API**: `POST /v1/ocr/scan`(不自然な箇所の検出)、`POST /v1/ocr/propose`(1 文字候補の提案)、`POST /v1/ocr/fill`(候補の採点・置換。`return_all` / `use_web` / `lm_candidates`)、`GET /v1/ocr/charset?n=`(頻出の漢字・かな)、`POST /v1/ocr/glyph-rank`(字形の照合、sgemm)。言語モデル(`/v1/qwen/install` → `/v1/qwen/select`)が必要(無いとき 503。`glyph-rank` は文書内の字だけでも動く)。検索は `ARUARU_LLM_SEARCH_URL`(既定 `http://127.0.0.1:4610` = `aruaru-search`、VPS 内からは回数制限なし。ローカルで試すときは SSH 中継 `-L 14610:127.0.0.1:4610`)。補完は手元 PC の `aruaru-llm` に任せるのが現実的(VPS は 4 コア・RAM 3.6GB)ため、設定 `fill_server_url` で OCR と別のサーバーを指定できる。

**試験のやり方**: 試験画像は Python(Pillow)で作った小さく薄くぼかしたかすれ文書(正解が分かる)。`RPoem/crates/open-runo-pdf-ocr/examples/e2e.rs`(`PDFOCR_REOCR=1 PDFOCR_FILL_WEB=0 PDFOCR_DEBUG=1 cargo run --release -p open-runo-pdf-ocr --example e2e -- in.pdf out.pdf <OCRサーバーURL> jpn+eng best 8 <補完サーバーURL>`)で OCR → 補完 → PDF を作り、`pdftotext` の結果を正解と比べる。`examples/vis_probe.rs` は字形の照合だけを、位置が既知の文書で測る(`JITTER=0.3` で位置ずれを与える。位置が既知なら 90 字種中の正解を 70〜91% で 1 位にできるが、ずれると下がる)。速度の測定: `cargo test --release --bin aruaru-llm bench_sgemm -- --ignored --nocapture`(aruaru-llm)。OCR は VPS の `aruaru-llm`(SSH 中継 `-L 14600:127.0.0.1:4600`)、補完は手元の `aruaru-llm`(Qwen2.5-1.5B)。Windows の bash から日本語を `curl` に直接渡すと文字化けするので、Python から UTF-8 で送る。環境変数は `export` してから起動する(サブシェルでの前置きが効かないことがあった)。

**未了・次にやること**: (1) 重く劣化したページで補完が効く条件の探索(修正を 1 か所ずつ確定して文脈を更新する反復、字形の確率 + 言語モデルの確率を合わせるビーム探索)、(2) 升目の位置の精度(行の枠からの按分はずれる)、(3) 補完の速度(小さい言語モデルでの先行絞り込み → 大きいモデルで確定、GPU の `sgemm`)、(4) 日本語向けのより大きいモデル(Qwen2.5-3B は分割 safetensors のため `QwenModel::load` の対応が要る)、(5) Android のかすれ補正の速度(実機で AI 超解像は 480×320 → FHD で約 51 秒)、(6) macOS / Linux 実機での確認。

### English
**What v1.3.0 contains (default behaviour)**
- **Automatic contrast correction (always on)**: faint or faded documents have so little difference between background and ink that they were classified as "blank" and never sent to OCR. The page is now stretched before OCR (pages with enough contrast are untouched). Character accuracy on the synthetic faded document: **0% → ~48%**.
- **Faded-text rescue (setting `reocr`, off by default)**: crop low-confidence lines → denoise → enlarge 3x with two readings (AI super-resolution `open-runo-sr` and interpolation) → OCR again and keep the more confident reading. On the same document **~48% → 89%**. AI super-resolution breaks glyphs into blocks unless the noise is removed first (measured), so a light blur always comes first.
- **AI context completion (setting `fill`, off by default, experimental)**: a language model (Qwen2.5) scans each line and flags unnatural 1–4 character spans → candidates from a look-alike table, glyph matching, search results (`aruaru-search`) and the model itself, scored in context and replaced. A replacement across several words merges them into one. When glyph evidence is strong, a candidate that is visually clearly worse than the original reading is never taken (precision first).
- **Glyph matching** (new shared crate `open-runo-glyph`): a faded cell is compared with candidate glyphs (serif and sans) by normalized cross-correlation. All positions of the glyph inside the cell become **one matrix product**, executed in the server's `POST /v1/ocr/glyph-rank` by `open-cuda`'s `sgemm` (CPU: `open-cpu` AVX2) — **~14 ms per cell** for ~9,200 glyphs × 289 positions. If the server does not support it the app falls back to the CPU reference (server first, so phones need not fetch fonts).
- **Qwen in `open-cuda`**: batched prefill, `score_continuations` (candidate log-likelihood), `top_next_tokens`, `token_logprobs`, and the vocabulary logits via `sgemm` (verified numerically identical to token-by-token decoding and to the threaded version).
- UI: "Faded-text rescue" and "AI context completion" settings (glyphs, search, server URL, max spots) on desktop and Android; the result message now reports re-read lines and completed spots.

**Measurements (honest record)**: a synthetic faded document (small, faint, blurred, noisy; 159 characters with known truth).

| Condition | Character accuracy | Time |
|---|---|---|
| Before (classified as blank, never OCR'd) | 0% | – |
| Automatic contrast correction only | ~48% | seconds |
| + faded-text rescue | 89.3% | ~20 s |
| + AI context completion (precision-first, Qwen2.5-1.5B, CPU) | 89.3% | ~220 s |
| Mildly faded document (98.7% already) with completion | 98.7% (no harm) | ~100 s |

On these tests, AI context completion **does no harm but adds nothing, and is slow** (10–20 s per spot; cost = model size × candidates × tokens). An intermediate version (multi-character spans plus unrestricted search) scored 87% in 880 s, i.e. **worse**, so the candidate count, the following context and the search targets were cut and the logic reworked for precision. A fully erased character remains a guess from context and can be wrong (e.g. "今□は" filled as "今朝は"). It therefore stays **off by default and experimental**.

**New server APIs (aruaru-llm)**: `POST /v1/ocr/scan` (detect unnatural spans), `POST /v1/ocr/propose` (single-character candidates), `POST /v1/ocr/fill` (score and replace; `return_all` / `use_web` / `lm_candidates`), `GET /v1/ocr/charset?n=` (frequent kanji/kana), `POST /v1/ocr/glyph-rank` (glyph matching via sgemm). They need a language model (`/v1/qwen/install` then `/v1/qwen/select`; 503 without one — `glyph-rank` works with just the characters found in the document). Search goes through `ARUARU_LLM_SEARCH_URL` (default `http://127.0.0.1:4610` = `aruaru-search`, unlimited from inside the VPS; for local tests use an SSH forward `-L 14610:127.0.0.1:4610`). Completion is realistically run on your own PC's `aruaru-llm` (the VPS has 4 cores / 3.6 GB RAM), so `fill_server_url` can point it at a different server than OCR.

**How it was tested**: the test images are small, faint, blurred documents made with Python (Pillow) so the truth is known. `RPoem/crates/open-runo-pdf-ocr/examples/e2e.rs` (`PDFOCR_REOCR=1 PDFOCR_FILL_WEB=0 PDFOCR_DEBUG=1 cargo run --release -p open-runo-pdf-ocr --example e2e -- in.pdf out.pdf <OCR server URL> jpn+eng best 8 <completion server URL>`) runs OCR → completion → PDF and the `pdftotext` result is compared with the truth. `examples/vis_probe.rs` measures glyph matching alone on a document with known positions (`JITTER=0.3` adds drift; with known positions the true character ranks first 70–91% of the time among 90 glyphs, lower when positions drift). Speed: `cargo test --release --bin aruaru-llm bench_sgemm -- --ignored --nocapture` (aruaru-llm). OCR used the VPS `aruaru-llm` (SSH forward `-L 14600:127.0.0.1:4600`), completion a local `aruaru-llm` (Qwen2.5-1.5B). Passing Japanese text to `curl` from Windows bash is garbled — send it from Python as UTF-8. `export` environment variables before launching (a sub-shell prefix sometimes did not take effect).

**Open items**: (1) find the conditions under which completion helps on heavily degraded pages (confirm one fix at a time and update the context; beam search combining glyph likelihood with language-model probability), (2) cell localization accuracy (apportioning the line box drifts), (3) completion speed (pre-filter with a small model, confirm with a larger one; GPU `sgemm`), (4) a larger Japanese-capable model (Qwen2.5-3B is sharded safetensors, which `QwenModel::load` does not support yet), (5) faded-text rescue speed on Android (on a real phone AI super-resolution takes ~51 s for 480×320 → FHD), (6) checks on real macOS / Linux machines.

---

作者 / Author: 石 塚 正 浩（MASAHIRO ISHIZUKA） (https://github.com/aon-co-jp)
