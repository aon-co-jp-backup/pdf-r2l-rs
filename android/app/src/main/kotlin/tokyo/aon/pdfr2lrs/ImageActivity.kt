package tokyo.aon.pdfr2lrs

import android.app.Activity
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
import android.widget.LinearLayout
import android.widget.RadioButton
import android.widget.RadioGroup
import android.widget.ScrollView
import android.widget.TextView

/** 写真・画像の高解像度化(AI超解像+補間)。1枚ずつ選び、保存先を決めて実行する。 */
class ImageActivity : Activity() {
    private val ui = Handler(Looper.getMainLooper())
    private var src: Uri? = null
    private var srcName = ""
    private var target = "4k"
    private var ai = true
    private var png = true
    @Volatile private var running = false
    private lateinit var fileLabel: TextView
    private lateinit var runButton: Button
    private lateinit var cancelButton: Button
    private lateinit var progressText: TextView
    private lateinit var resultText: TextView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val pad = dp(16)
        val root = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(pad, pad, pad, pad) }
        val primary = android.util.TypedValue().also { theme.resolveAttribute(android.R.attr.textColorPrimary, it, true) }.let { getColor(it.resourceId) }
        fun label(s: String, size: Float = 14f, bold: Boolean = false) = TextView(this).apply {
            text = s; setTextColor(primary); textSize = size
            if (bold) setTypeface(typeface, Typeface.BOLD)
        }
        fun add(v: View) = root.addView(v, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
        fun radio(base: Int, items: List<Pair<String, String>>, current: String, on: (String) -> Unit) = RadioGroup(this).apply {
            for ((i, it) in items.withIndex()) addView(RadioButton(this@ImageActivity).apply { id = base + i; text = it.second; isChecked = it.first == current })
            setOnCheckedChangeListener { _, c -> on(items[c - base].first) }
        }

        add(label("画像の高解像度化 / Upscale an image", 20f, true))
        add(Button(this).apply {
            text = "画像を選択 / Choose an image"
            setOnClickListener {
                startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                    addCategory(Intent.CATEGORY_OPENABLE); type = "image/*"
                    putExtra(Intent.EXTRA_MIME_TYPES, arrayOf("image/png", "image/jpeg"))
                }, REQ_PICK)
            }
        })
        fileLabel = label("(未選択 / none)")
        add(fileLabel)
        add(label("目標の解像度(縦横比を保って枠に収めます) / Target (fits the frame, keeps aspect ratio):", 13f, true))
        add(radio(100, listOf(
            "fhd" to "フルHD 1920×1080 / Full HD", "qhd" to "QHD 2560×1440(フルHDと4Kの間 / between FHD and 4K)",
            "4k" to "4K 3840×2160", "5k" to "5K 5120×2880(4Kと8Kの間 / between 4K and 8K)",
            "8k" to "8K 7680×4320", "16k" to "16K 15360×8640"), target) { target = it })
        add(radio(200, listOf("ai" to "AI超解像で細部を補う(おすすめ・遅い) / AI super-resolution (recommended, slow)",
            "plain" to "高品質な補間のみ(速い) / Interpolation only (fast)"), "ai") { ai = it == "ai" })
        add(radio(300, listOf("png" to "PNG(可逆・大きい) / PNG (lossless, large)", "jpg" to "JPEG(小さい) / JPEG (small)"), "png") { png = it == "png" })
        add(label("元の画像は変更しません。AI超解像は失われた細部を推測して補うもので、実際の細部の復元ではありません。スマホ(Arm)のCPUでは大変遅く、8K・16Kはメモリを大量に使うため、まず小さい目標でお試しください。\n" +
            "The original is untouched. AI super-resolution guesses missing detail; it is not a restoration. It is very slow on phone (Arm) CPUs, and 8K/16K need a lot of memory - try a smaller target first.", 11f))

        val bottom = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(pad, dp(4), pad, dp(4)) }
        val row = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
        runButton = Button(this).apply { text = "高解像度化して保存… / Upscale and save…"; setOnClickListener { askDestination() } }
        row.addView(runButton, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 2f))
        cancelButton = Button(this).apply { text = "キャンセル / Cancel"; visibility = View.GONE; setOnClickListener { Native.cancel() } }
        row.addView(cancelButton, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
        bottom.addView(row)
        progressText = label("").apply { visibility = View.GONE }
        resultText = label("").apply { visibility = View.GONE }
        bottom.addView(progressText); bottom.addView(resultText)

        val maxW = dp(640)
        val column = android.widget.FrameLayout(this).apply {
            addView(root, android.widget.FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, android.view.Gravity.CENTER_HORIZONTAL))
        }
        val container = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            addView(ScrollView(this@ImageActivity).apply { addView(column) }, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f))
            addView(bottom, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
        }
        container.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ ->
            val w = minOf(resources.displayMetrics.widthPixels, maxW)
            (root.layoutParams as android.widget.FrameLayout.LayoutParams).width = w
            root.requestLayout()
            val side = maxOf(0, (resources.displayMetrics.widthPixels - w) / 2)
            (bottom.layoutParams as LinearLayout.LayoutParams).apply { leftMargin = side; rightMargin = side }
            bottom.requestLayout()
        }
        container.setOnApplyWindowInsetsListener { _, ins ->
            @Suppress("DEPRECATION")
            container.setPadding(ins.systemWindowInsetLeft, ins.systemWindowInsetTop, ins.systemWindowInsetRight, ins.systemWindowInsetBottom)
            ins
        }
        setContentView(container)
        intent?.data?.let { setSource(it) }
    }

    private fun dp(v: Int) = (v * resources.displayMetrics.density).toInt()

    private fun setSource(uri: Uri) {
        src = uri
        srcName = contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { if (it.moveToFirst()) it.getString(0) else null } ?: "image.png"
        fileLabel.text = srcName
    }

    private fun askDestination() {
        if (src == null) { resultText.visibility = View.VISIBLE; resultText.text = "先に画像を選んでください。/ Choose an image first."; resultText.setTextColor(Color.rgb(200, 40, 40)); return }
        val stem = srcName.substringBeforeLast('.')
        startActivityForResult(Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = if (png) "image/png" else "image/jpeg"
            putExtra(Intent.EXTRA_TITLE, "$stem-$target." + if (png) "png" else "jpg")
        }, REQ_SAVE)
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        val uri = data?.data
        if (resultCode != RESULT_OK || uri == null) return
        if (requestCode == REQ_PICK) setSource(uri) else if (requestCode == REQ_SAVE) run(uri)
    }

    private fun run(dest: Uri) {
        val from = src ?: return
        if (running) return
        running = true
        runButton.isEnabled = false; cancelButton.visibility = View.VISIBLE
        resultText.visibility = View.GONE
        progressText.visibility = View.VISIBLE; progressText.text = "開始中… / Starting…"
        val poll = object : Runnable {
            override fun run() {
                if (!running) return
                val p = Native.progress().split("|", limit = 3)
                if (p.size == 3 && p[0] == "image") progressText.text = "処理中 / Working… ${p[1]}%  ${p[2]}"
                ui.postDelayed(this, 500)
            }
        }
        ui.postDelayed(poll, 500)
        val t = target
        val a = ai
        val f = png
        Thread {
            var ok = true
            val msg = try {
                val input = contentResolver.openInputStream(from)!!.use { it.readBytes() }
                val out = Native.upscale(input, t, a, f)
                contentResolver.openOutputStream(dest, "wt")!!.use { it.write(out) }
                Native.message()
            } catch (e: Throwable) { ok = false; e.message ?: "error" }
            running = false
            ui.post {
                runButton.isEnabled = true; cancelButton.visibility = View.GONE; progressText.visibility = View.GONE
                resultText.visibility = View.VISIBLE
                resultText.text = if (ok) msg else "エラー / Error: $msg"
                resultText.setTextColor(if (ok) Color.rgb(0, 140, 60) else Color.rgb(200, 40, 40))
            }
        }.start()
    }

    companion object {
        private const val REQ_PICK = 1
        private const val REQ_SAVE = 2
    }
}
