package tokyo.aon.pdfr2lrs

import android.app.Activity
import android.app.AlertDialog
import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.graphics.Typeface
import android.net.Uri
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.provider.OpenableColumns
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.CheckBox
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.RadioButton
import android.widget.RadioGroup
import android.widget.ScrollView
import android.widget.TextView
import org.json.JSONArray
import org.json.JSONObject

/** PDF R2L RS(Android 版)。PDF を選び、綴じ方向・見開き・OCR の設定を決めて、別名で保存する。 */
class MainActivity : Activity() {
    private val ui = Handler(Looper.getMainLooper())
    private lateinit var prefs: JSONObject
    private var src: Uri? = null
    private var srcName = ""
    @Volatile private var running = false

    private lateinit var fileLabel: TextView
    private lateinit var convertButton: Button
    private lateinit var cancelButton: Button
    private lateinit var progressText: TextView
    private lateinit var resultText: TextView
    private lateinit var fallbackButton: Button
    private lateinit var ocrPanel: LinearLayout
    private lateinit var languageLabel: TextView
    private lateinit var testResult: TextView
    private var ocrOverride: Boolean? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        prefs = loadPrefs()
        val pad = dp(16)
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(pad, pad, pad, pad)
        }
        fun add(v: View) = root.addView(v, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
        val primary = android.util.TypedValue().also { theme.resolveAttribute(android.R.attr.textColorPrimary, it, true) }.let { getColor(it.resourceId) }
        fun label(s: String, size: Float = 14f, bold: Boolean = false) = TextView(this).apply {
            text = s
            setTextColor(primary)
            textSize = size
            if (bold) setTypeface(typeface, Typeface.BOLD)
        }

        add(label("PDF 綴じ方向変更 / PDF binding direction", 20f, true))
        add(Button(this).apply {
            text = "画像の高解像度化 / Upscale images (FHD〜16K)"
            setOnClickListener { startActivity(Intent(this@MainActivity, ImageActivity::class.java)) }
        })
        add(Button(this).apply {
            text = "PDFを選択 / Choose PDF"
            setOnClickListener {
                startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                    addCategory(Intent.CATEGORY_OPENABLE)
                    type = "application/pdf"
                }, REQ_PICK)
            }
        })
        fileLabel = label("(未選択 / none)")
        add(fileLabel)

        // 綴じ方向: 右綴じ / 左綴じ / 元と同じ
        val binding = RadioGroup(this)
        val choices = listOf("r2l" to "右綴じ (R2L) / Right binding", "l2r" to "左綴じ (L2R) / Left binding", "keep" to "元と同じ(変更しない) / Same as the original (no change)")
        val spread = CheckBox(this).apply { text = "見開き表示にする / Show as two-page spread" }
        for ((i, c) in choices.withIndex()) {
            binding.addView(RadioButton(this).apply {
                id = 100 + i
                text = c.second
                isChecked = prefs.optString("binding", "r2l") == c.first
            })
        }
        binding.setOnCheckedChangeListener { _, checked ->
            prefs.put("binding", choices[checked - 100].first)
            spread.isEnabled = choices[checked - 100].first != "keep"
            savePrefs()
        }
        add(binding)
        spread.isChecked = prefs.optBoolean("spread", true)
        spread.isEnabled = prefs.optString("binding", "r2l") != "keep"
        spread.setOnCheckedChangeListener { _, v -> prefs.put("spread", v); savePrefs() }
        add(spread)

        // OCR
        val ocr = prefs.getJSONObject("ocr")
        add(CheckBox(this).apply {
            text = "OCRして、元の書体に合わせた検索できる文字に置き換える / OCR and replace with searchable text in the matching typeface"
            isChecked = ocr.optBoolean("enabled", true)
            setOnCheckedChangeListener { _, v ->
                ocr.put("enabled", v); savePrefs()
                ocrPanel.visibility = if (v) View.VISIBLE else View.GONE
            }
        })
        val info = JSONObject(Native.languagesJson())
        add(label(info.getString("feature_ja") + "\n" + info.getString("feature_en"), 11f))

        ocrPanel = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            visibility = if (ocr.optBoolean("enabled", true)) View.VISIBLE else View.GONE
        }
        ocrPanel.addView(label("接続先URL / Server URL(エミュレーターでは / on the emulator: http://10.0.2.2:4600)", 12f))
        val url = EditText(this).apply {
            setText(ocr.optString("server_url", "http://127.0.0.1:4600"))
            setSingleLine()
            addTextChangedListener(object : android.text.TextWatcher {
                override fun afterTextChanged(s: android.text.Editable?) { ocr.put("server_url", s.toString().trim()); savePrefs() }
                override fun beforeTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) {}
                override fun onTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) {}
            })
        }
        ocrPanel.addView(url)
        testResult = label("")
        ocrPanel.addView(Button(this).apply {
            text = "接続テスト / Test connection"
            setOnClickListener {
                testResult.setTextColor(Color.GRAY); testResult.text = "接続を確認中… / Testing…"
                Thread {
                    val r = try { Native.testConnection(ocr.optString("server_url")) to true } catch (e: Throwable) { (e.message ?: "error") to false }
                    ui.post { testResult.text = r.first; testResult.setTextColor(if (r.second) Color.rgb(0, 140, 60) else Color.rgb(200, 40, 40)) }
                }.start()
            }
        })
        ocrPanel.addView(testResult)
        languageLabel = label("")
        ocrPanel.addView(Button(this).apply { text = "言語を選ぶ… / Choose languages…"; setOnClickListener { chooseLanguages(info) } })
        ocrPanel.addView(languageLabel)
        updateLanguageLabel(info)

        ocrPanel.addView(label("品質 / Quality:", 13f, true))
        ocrPanel.addView(radio(listOf("best" to "高精度 / Best", "fast" to "高速 / Fast"), ocr.optString("quality", "best")) { ocr.put("quality", it); savePrefs() })
        ocrPanel.addView(label(info.getString("quality_help_ja") + "\n" + info.getString("quality_help_en"), 11f))
        ocrPanel.addView(label("文字の扱い / Text handling:", 13f, true))
        ocrPanel.addView(radio(
            listOf("replace" to "元の書体に合わせた文字に置き換える / Replace with text in the matching typeface", "search" to "元の画像はそのまま、検索用の見えない文字だけ追加 / Keep the original image; add invisible searchable text only"),
            if (ocr.optBoolean("replace_text", true)) "replace" else "search",
        ) { ocr.put("replace_text", it == "replace"); savePrefs() })
        ocrPanel.addView(label("AI文脈補完 / AI context completion:", 13f, true))
        ocrPanel.addView(CheckBox(this).apply {
            text = "かすれて読めない1〜2文字を、前後の文脈から推測して穴埋め / Guess one or two faded characters from the surrounding text"
            isChecked = ocr.optBoolean("fill", false)
            setOnCheckedChangeListener { _, v -> ocr.put("fill", v); savePrefs() }
        })
        ocrPanel.addView(label("低信頼の短い単語・欠けた文字・余計な1文字だけが対象で、長文は書き換えません。推測なので誤ることがあります。サーバーに言語モデル(例: Qwen2.5)が必要です。\nOnly low-confidence short words, missing characters and stray characters are considered; long text is never rewritten. It is a guess and can be wrong. The server needs a language model (e.g. Qwen2.5).", 11f))
        ocrPanel.addView(label("補完サーバーURL(空ならOCRと同じ) / Completion server URL (blank = same as OCR)", 12f))
        ocrPanel.addView(EditText(this).apply {
            setText(ocr.optString("fill_server_url", ""))
            setSingleLine()
            addTextChangedListener(object : android.text.TextWatcher {
                override fun afterTextChanged(s: android.text.Editable?) { ocr.put("fill_server_url", s.toString().trim()); savePrefs() }
                override fun beforeTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) {}
                override fun onTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) {}
            })
        })
        add(ocrPanel)

        // 変換ボタン・進捗・結果は、設定が長くても常に見えるよう、画面下に固定する。
        val bottom = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(pad, dp(4), pad, dp(4)) }
        fun addBottom(v: View) = bottom.addView(v, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
        // ボタンは1行(変換・キャンセル・ライセンス)。メッセージは、空のときは場所を取らない。
        fun autoHide(tv: TextView) = tv.apply {
            visibility = View.GONE
            addTextChangedListener(object : android.text.TextWatcher {
                override fun afterTextChanged(s: android.text.Editable?) { visibility = if (s.isNullOrEmpty()) View.GONE else View.VISIBLE }
                override fun beforeTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) {}
                override fun onTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) {}
            })
        }
        val row = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
        fun rowButton(b: Button, weight: Float) = row.addView(b, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, weight))
        convertButton = Button(this).apply {
            text = "変換して保存… / Convert and save…"
            setOnClickListener { askDestination() }
        }
        rowButton(convertButton, 2f)
        cancelButton = Button(this).apply { text = "キャンセル / Cancel"; visibility = View.GONE; setOnClickListener { Native.cancel() } }
        rowButton(cancelButton, 1f)
        rowButton(Button(this).apply { text = "ライセンス / License"; setOnClickListener { showLicense() } }, 1f)
        addBottom(row)
        progressText = autoHide(label(""))
        addBottom(progressText)
        resultText = autoHide(label(""))
        addBottom(resultText)
        fallbackButton = Button(this).apply {
            text = "OCRを使わずに、綴じ方向だけ変換 / Convert without OCR (direction only)"
            visibility = View.GONE
            setOnClickListener { ocrOverride = false; askDestination() }
        }
        addBottom(fallbackButton)

        val maxW = dp(640)
        val column = android.widget.FrameLayout(this).apply {
            addView(root, android.widget.FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, android.view.Gravity.CENTER_HORIZONTAL))
        }
        val scroll = ScrollView(this).apply { addView(column) }
        val container = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            addView(scroll, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f))
            addView(bottom, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
        }
        // 広い画面(タブレット・横向き)では、内容の幅を最大 640dp にして中央に寄せる。
        fun limitWidth() {
            val w = minOf(resources.displayMetrics.widthPixels, maxW)
            (root.layoutParams as android.widget.FrameLayout.LayoutParams).width = w
            root.requestLayout()
            val bw = (bottom.layoutParams as LinearLayout.LayoutParams)
            val side = maxOf(0, (resources.displayMetrics.widthPixels - w) / 2)
            bw.leftMargin = side; bw.rightMargin = side
            bottom.requestLayout()
        }
        container.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ -> limitWidth() }
        // Android 15 の画面端までの描画で、バー・ノッチに隠れないよう余白を足す(縦横どちらでも)。
        container.setOnApplyWindowInsetsListener { _, ins ->
            ("DEPRECATION")
            container.setPadding(ins.systemWindowInsetLeft, ins.systemWindowInsetTop, ins.systemWindowInsetRight, ins.systemWindowInsetBottom)
            ins
        }
        setContentView(container)
        // 他のアプリから「共有/開く」で渡された PDF
        intent?.data?.let { setSource(it) }
        debugRun()
    }

    /** デバッグビルド専用の試験用フック: `--es debug_input <filesDir内のPDF> --es debug_output <保存先>` で、画面を使わずに変換して結果を logcat(PDFR2L)へ出す。 */
    private fun debugRun() {
        val debuggable = (applicationInfo.flags and android.content.pm.ApplicationInfo.FLAG_DEBUGGABLE) != 0
        intent?.getStringExtra("debug_image")?.let { img ->
            if (!debuggable) return
            val target = intent?.getStringExtra("debug_target") ?: "fhd"
            Thread {
                val t = System.currentTimeMillis()
                try {
                    val out = Native.upscale(java.io.File(filesDir, img).readBytes(), target, intent?.getBooleanExtra("debug_ai", true) ?: true, true)
                    java.io.File(filesDir, "$img.$target.png").writeBytes(out)
                    android.util.Log.i("PDFR2L", "IMG DONE in " + (System.currentTimeMillis() - t) + "ms: " + Native.message().replace(Char(10), '|'))
                } catch (e: Throwable) {
                    android.util.Log.e("PDFR2L", "IMG FAILED: " + e.message)
                }
            }.start()
            return
        }
        val input = intent?.getStringExtra("debug_input")
        val output = intent?.getStringExtra("debug_output")
        if (!debuggable || input == null || output == null) return
        intent?.getStringExtra("debug_url")?.let { prefs.getJSONObject("ocr").put("server_url", it) }
        Thread {
            val t = System.currentTimeMillis()
            try {
                val bytes = java.io.File(filesDir, input).readBytes()
                val out = Native.convert(bytes, prefs.toString(), filesDir.absolutePath)
                java.io.File(filesDir, output).writeBytes(out)
                android.util.Log.i("PDFR2L", "DONE in " + (System.currentTimeMillis() - t) + "ms: " + Native.message().replace(Char(10), '|'))
            } catch (e: Throwable) {
                android.util.Log.e("PDFR2L", "FAILED: " + e.message)
            }
        }.start()
    }

    private fun dp(v: Int) = (v * resources.displayMetrics.density).toInt()

    private fun radio(items: List<Pair<String, String>>, current: String, onChange: (String) -> Unit): RadioGroup {
        val g = RadioGroup(this)
        for ((i, it) in items.withIndex()) {
            g.addView(RadioButton(this).apply { id = 500 + i; text = it.second; isChecked = it.first == current })
        }
        g.setOnCheckedChangeListener { _, checked -> onChange(items[checked - 500].first) }
        return g
    }

    private fun loadPrefs(): JSONObject {
        val defaults = JSONObject(Native.defaultsJson())
        val saved = getSharedPreferences("prefs", Context.MODE_PRIVATE).getString("json", null)
        val p = try { if (saved != null) JSONObject(saved) else JSONObject() } catch (e: Exception) { JSONObject() }
        if (!p.has("binding")) p.put("binding", "r2l")
        if (!p.has("spread")) p.put("spread", true)
        val ocr = p.optJSONObject("ocr") ?: JSONObject()
        for (k in defaults.keys()) if (!ocr.has(k)) ocr.put(k, defaults.get(k))
        p.put("ocr", ocr)
        return p
    }

    private fun savePrefs() {
        getSharedPreferences("prefs", Context.MODE_PRIVATE).edit().putString("json", prefs.toString()).apply()
    }

    private fun selectedLanguages(): MutableList<String> {
        val a = prefs.getJSONObject("ocr").getJSONArray("languages")
        return MutableList(a.length()) { a.getString(it) }
    }

    private fun updateLanguageLabel(info: JSONObject) {
        val names = info.getJSONArray("languages")
        val byCode = HashMap<String, String>()
        for (i in 0 until names.length()) names.getJSONObject(i).let { byCode[it.getString("code")] = it.getString("ja") + " / " + it.getString("en") }
        val sel = selectedLanguages()
        languageLabel.text = "言語 / Languages (${sel.size}/12): " + sel.joinToString("、 ") { byCode[it] ?: it }
    }

    /** 言語選択(1〜12個)。日英の説明つき。 */
    private fun chooseLanguages(info: JSONObject) {
        val langs: JSONArray = info.getJSONArray("languages")
        val selected = selectedLanguages().toMutableSet()
        val count = TextView(this)
        val boxes = ArrayList<CheckBox>()
        fun refresh() {
            count.text = "選択中 / Selected: ${selected.size}/12"
            for (b in boxes) b.isEnabled = b.isChecked || selected.size < 12
        }
        val list = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(dp(16), dp(8), dp(16), dp(8)) }
        list.addView(TextView(this).apply { text = info.getString("language_help_ja") + "\n\n" + info.getString("language_help_en"); textSize = 12f })
        list.addView(count)
        for (i in 0 until langs.length()) {
            val l = langs.getJSONObject(i)
            val code = l.getString("code")
            val cb = CheckBox(this).apply {
                text = l.getString("ja") + " / " + l.getString("en") + "  ($code)"
                isChecked = code in selected
                setOnCheckedChangeListener { _, v ->
                    if (v) selected.add(code) else if (selected.size > 1) selected.remove(code) else isChecked = true
                    refresh()
                }
            }
            boxes.add(cb)
            list.addView(cb)
        }
        refresh()
        AlertDialog.Builder(this)
            .setTitle("言語を選ぶ / Choose languages")
            .setView(ScrollView(this).apply { addView(list) })
            .setPositiveButton("OK") { _, _ ->
                val order = (0 until langs.length()).map { langs.getJSONObject(it).getString("code") }.filter { it in selected }
                prefs.getJSONObject("ocr").put("languages", JSONArray(order))
                savePrefs(); updateLanguageLabel(info)
            }
            .setNegativeButton("Cancel / キャンセル", null)
            .show()
    }

    private fun showLicense() {
        val text = assets.open("LICENSES.txt").bufferedReader(Charsets.UTF_8).use { it.readText() }.trimStart('﻿')
        AlertDialog.Builder(this)
            .setTitle("ライセンス / License")
            .setView(ScrollView(this).apply {
                addView(TextView(this@MainActivity).apply {
                    this.text = text; textSize = 11f; typeface = Typeface.MONOSPACE; setTextIsSelectable(true); setPadding(dp(12), dp(12), dp(12), dp(12))
                })
            })
            .setPositiveButton("OK", null)
            .show()
    }

    private fun setSource(uri: Uri) {
        src = uri
        srcName = contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { if (it.moveToFirst()) it.getString(0) else null } ?: "document.pdf"
        fileLabel.text = srcName
    }

    private fun suggestName(): String {
        val stem = srcName.removeSuffix(".pdf").removeSuffix(".PDF")
        val suffix = when (prefs.optString("binding")) { "l2r" -> "l2r"; "keep" -> "ocr"; else -> "r2l" }
        return "$stem-$suffix.pdf"
    }

    private fun askDestination() {
        if (src == null) { resultText.text = "先にPDFを選んでください。/ Choose a PDF first."; resultText.setTextColor(Color.rgb(200, 40, 40)); return }
        startActivityForResult(Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = "application/pdf"
            putExtra(Intent.EXTRA_TITLE, suggestName())
        }, REQ_SAVE)
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        val uri = data?.data
        if (resultCode != RESULT_OK || uri == null) { ocrOverride = null; return }
        when (requestCode) {
            REQ_PICK -> setSource(uri)
            REQ_SAVE -> convert(uri)
        }
    }

    private fun progressLabel(raw: String): String {
        val p = raw.split("|")
        if (p.size < 3) return "開始中… / Starting…"
        return when (p[0]) {
            "prepare" -> "言語データを準備中… / Preparing language data… (初回は時間がかかります / the first time takes a while)"
            "ocr" -> "OCR中 / Reading text… ${p[1]}/${p[2]} ページ / pages"
            else -> "書体を取得して PDF を組み立て中… / Fetching fonts and building the PDF…"
        }
    }

    private fun convert(dest: Uri) {
        val from = src ?: return
        if (running) return
        running = true
        val use = JSONObject(prefs.toString())
        ocrOverride?.let { use.getJSONObject("ocr").put("enabled", it) }
        ocrOverride = null
        convertButton.isEnabled = false; cancelButton.visibility = View.VISIBLE; fallbackButton.visibility = View.GONE
        resultText.text = ""; progressText.text = "開始中… / Starting…"
        val poll = object : Runnable {
            override fun run() { if (running) { progressText.text = progressLabel(Native.progress()); ui.postDelayed(this, 400) } }
        }
        ui.postDelayed(poll, 400)
        Thread {
            var ok = true
            val msg = try {
                val input = contentResolver.openInputStream(from)!!.use { it.readBytes() }
                val out = Native.convert(input, use.toString(), filesDir.absolutePath)
                contentResolver.openOutputStream(dest, "wt")!!.use { it.write(out) }
                Native.message().replaceFirst("Done: ", "Done: $srcName → ")
            } catch (e: Throwable) { ok = false; e.message ?: "error" }
            running = false
            ui.post {
                convertButton.isEnabled = true; cancelButton.visibility = View.GONE; progressText.text = ""
                resultText.text = if (ok) msg else "エラー / Error: $msg"
                resultText.setTextColor(if (ok) Color.rgb(0, 140, 60) else Color.rgb(200, 40, 40))
                if (!ok && msg.contains("online environment")) fallbackButton.visibility = View.VISIBLE
            }
        }.start()
    }

    companion object {
        private const val REQ_PICK = 1
        private const val REQ_SAVE = 2
    }
}
