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
