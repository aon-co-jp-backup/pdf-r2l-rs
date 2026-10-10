package tokyo.aon.pdfr2lrs

/** Rust 製の中核(libpdfr2l_jni.so)への橋渡し。 */
object Native {
    init {
        System.loadLibrary("pdfr2l_jni")
    }

    /** PDF を変換して返す(失敗時は RuntimeException)。 */
    @JvmStatic external fun convert(input: ByteArray, prefsJson: String, cacheDir: String): ByteArray
    /** "stage|done|total"(stage: prepare / ocr / compose)。 */
    @JvmStatic external fun progress(): String
    @JvmStatic external fun message(): String
    @JvmStatic external fun cancel()
    @JvmStatic external fun defaultsJson(): String
    @JvmStatic external fun languagesJson(): String
    @JvmStatic external fun testConnection(url: String): String
}
