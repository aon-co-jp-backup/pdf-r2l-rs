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

## 次版の予定: AI文脈補正 / Next: AI context completion

**日本語**: OCRでかすれて読めない文字を、**前後の長い文脈から推測して1〜2文字だけ穴埋めする**機能です(長文の書き換えではありません)。
方針: Tesseractの低信頼の文字と代替候補を取り出し、前後の文脈つきで言語モデルが各候補の尤もらしさを採点して選ぶ(`open-cpu` / `open-cuda` の推論エンジンで
候補の対数尤度を返すAPIが必要。現在の `POST /v1/ocr/correct` は行単位の書き換え方式で、試験の結果、実用速度に届かないため実験扱い)。

**English**: Fill in just one or two smudged/unreadable characters using the long surrounding context (not long-text rewriting). Plan: take low-confidence characters and
alternative candidates from Tesseract, then let a language model score each candidate in context (needs a log-likelihood API in the open-cpu / open-cuda engines). The current
line-rewrite endpoint `POST /v1/ocr/correct` is experimental because it is too slow in practice.

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
