

# pdf-r2l-rs(開発ルール・引き継ぎ) / Development rules & handoff

- 共通ルール(UI・インストーラーの試験、日本語での応答、ライセンスの扱い)は `F:\runo\CLAUDE.md` と `open-raid-z/CLAUDE.md` に従う。 / Common rules live in `F:\runo\CLAUDE.md` and `open-raid-z/CLAUDE.md`.
- GUI・Android を変えたら `docs/UI-TEST-CHECKLIST.md` と `scripts/ui-test/` で、複数の解像度・縦横・実機で確認する(スマホ実機は USB 接続のものを使える)。 / After UI changes, run the checklist and scripts across resolutions/orientations/real devices.
- 共有ロジックは RPoem(`open-runo-pdf-ocr` / `open-runo-sr` / `open-runo-glyph`)、サーバーは `aruaru-llm`、推論エンジンは `open-cuda`。`Cargo.toml` の RPoem は `rev` で固定する。 / Shared logic is in RPoem, the server is `aruaru-llm`, the engine is `open-cuda`; pin the RPoem `rev`.
- **AI 文脈補完は実験的・既定オフ**(測定では害はないが上積みも無く遅い)。通し測定せずにオンにしたり、成果として報告したりしない。 / AI context completion is experimental and off by default (no harm but no gain, and slow, in measurements); never enable it or report it as a win without end-to-end measurements.

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
