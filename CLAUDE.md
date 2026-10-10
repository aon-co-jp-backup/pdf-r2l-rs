# pdf-r2l-rs(開発ルール・引き継ぎ) / Development rules & handoff

- 共通ルール(UI・インストーラーの試験、日本語での応答、ライセンスの扱い)は `F:\runo\CLAUDE.md` と `open-raid-z/CLAUDE.md` に従う。 / Common rules live in `F:\runo\CLAUDE.md` and `open-raid-z/CLAUDE.md`.
- GUI・Android を変えたら `docs/UI-TEST-CHECKLIST.md` と `scripts/ui-test/` で、複数の解像度・縦横・実機で確認する(スマホ実機は USB 接続のものを使える)。 / After UI changes, run the checklist and scripts across resolutions/orientations/real devices.
- 共有ロジックは RPoem(`open-runo-pdf-ocr` / `open-runo-sr` / `open-runo-glyph`)、サーバーは `aruaru-llm`、推論エンジンは `open-cuda`。`Cargo.toml` の RPoem は `rev` で固定する。 / Shared logic is in RPoem, the server is `aruaru-llm`, the engine is `open-cuda`; pin the RPoem `rev`.
- **AI 文脈補完は実験的・既定オフ**。通し測定で悪化が確認されているので、結果を検証せずにオンにしたり、成果として報告したりしない。 / AI context completion is experimental and off by default; do not enable it or report it as a win without end-to-end measurements.

## 2026-10-10 開発停止時点の記録(OCR かすれ補正・AI 文脈補完) / Status at stop (faded-text rescue & AI context completion)

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
