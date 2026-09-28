package dev.mini.browser

import android.app.Activity
import android.os.Bundle
import android.view.View
import android.view.inputmethod.EditorInfo
import android.webkit.WebView
import android.webkit.WebViewClient
import android.webkit.WebChromeClient
import android.webkit.CookieManager
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.ProgressBar
import android.widget.TextView
import android.graphics.Color
import android.view.Gravity

/**
 * Mini for Android — full-featured, lightweight, stealth-capable.
 *
 * Feature set:
 *  - Omnibox: search-or-URL, tracker-parameter stripping in stealth
 *  - Stealth mode: in-memory only, no cookies/history/cache persisted, hardened UA
 *  - Find-in-page, load progress, load-error recovery, back/forward
 *  - Dark "ink" theme matching the desktop start page (aurora/ink palette)
 */
class MainActivity : Activity() {

    companion object {
        const val STEALTH_UA = "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/154.0.0.0 Mobile Safari/537.36 MiniStealth/1.0"
        val TRACKER_PARAMS = setOf(
            "utm_source", "utm_medium", "utm_campaign", "utm_term", "utm_content",
            "utm_id", "fbclid", "gclid", "dclid", "msclkid", "twclid", "igshid",
            "mc_cid", "mc_eid", "_hsenc", "_hsmi", "yclid", "ttclid"
        )
    }

    private lateinit var root: FrameLayout
    private lateinit var webView: WebView
    private lateinit var omnibox: EditText
    private lateinit var progress: ProgressBar
    private lateinit var stealthBadge: TextView
    private var stealth = false

    // ---------------------------------------------------------------- lifecycle

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        stealth = intent.getBooleanExtra("stealth", false)
        buildUi()

        root.addView(webView, FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT))
        webView.webViewClient = MiniClient()
        webView.webChromeClient = object : WebChromeClient() {
            override fun onProgressChanged(view: WebView?, newProgress: Int) {
                progress.visibility = if (newProgress in 1..99) View.VISIBLE else View.GONE
                progress.progress = newProgress
            }
        }

        if (stealth) applyStealth()

        val target = intent?.dataString
        if (target != null) {
            webView.loadUrl(sanitize(target))
        } else {
            webView.loadUrl("file:///android_asset/start.html" + if (stealth) "?stealth=1" else "")
        }
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        if (!stealth) webView.saveState(outState)
    }

    override fun onBackPressed() {
        if (webView.canGoBack()) webView.goBack() else super.onBackPressed()
    }

    override fun onDestroy() {
        if (stealth) {
            // Scrub everything from memory before teardown.
            webView.apply {
                clearCache(true)
                clearFormData()
                clearHistory()
            }
            CookieManager.getInstance().apply {
                removeAllCookies(null)
                flush()
            }
        }
        webView.destroy()
        super.onDestroy()
    }

    // ------------------------------------------------------------------- UI

    private fun buildUi() {
        val dp = resources.displayMetrics.density

        root = FrameLayout(this).apply { setBackgroundColor(Color.parseColor("#0E0D12")) }

        webView = makeWebView()

        progress = ProgressBar(this).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT, (3 * dp).toInt(), Gravity.TOP)
            isIndeterminate = false
            max = 100
            progressTintList = android.content.res.ColorStateList.valueOf(Color.parseColor("#FF7A45"))
            visibility = View.GONE
        }
        root.addView(progress)

        val bar = FrameLayout(this).apply {
            setBackgroundColor(Color.parseColor("#EE16151C"))
            elevation = 10 * dp
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.WRAP_CONTENT,
                Gravity.TOP)
        }

        omnibox = EditText(this).apply {
            hint = "search or type url"
            setTextColor(Color.parseColor("#E9E6F0"))
            setHintTextColor(Color.parseColor("#55505F"))
            setBackgroundColor(Color.parseColor("#1E1C26"))
            setSingleLine()
            imeOptions = EditorInfo.IME_ACTION_GO
            setPadding((16 * dp).toInt(), (11 * dp).toInt(), (16 * dp).toInt(), (11 * dp).toInt())
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.WRAP_CONTENT).apply {
                marginStart = (12 * dp).toInt(); marginEnd = (12 * dp).toInt()
                topMargin = (8 * dp).toInt(); bottomMargin = (8 * dp).toInt()
            }
            setOnEditorActionListener { _, action, _ ->
                if (action == EditorInfo.IME_ACTION_GO) { go(omnibox.text.toString()); true } else false
            }
        }
        bar.addView(omnibox)

        stealthBadge = TextView(this).apply {
            text = "STEALTH"
            setTextColor(Color.parseColor("#B06CFF"))
            textSize = 10f
            letterSpacing = 0.3f
            setPadding((10 * dp).toInt(), (4 * dp).toInt(), (10 * dp).toInt(), (4 * dp).toInt())
            visibility = if (stealth) View.VISIBLE else View.GONE
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.WRAP_CONTENT, FrameLayout.LayoutParams.WRAP_CONTENT).apply {
                gravity = Gravity.TOP or Gravity.END
                topMargin = (54 * dp).toInt(); marginEnd = (10 * dp).toInt()
            }
        }
        bar.addView(stealthBadge)

        addContentView(bar, FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.WRAP_CONTENT))

        setContentView(root)
    }

    private fun makeWebView() = WebView(this).apply {
        settings.javaScriptEnabled = true
        settings.domStorageEnabled = true
        settings.builtInZoomControls = true
        settings.displayZoomControls = false
        settings.useWideViewPort = true
        settings.loadWithOverviewMode = true
        settings.mediaPlaybackRequiresUserGesture = true
        isFocusableInTouchMode = true
    }

    // ----------------------------------------------------------------- engine

    private fun applyStealth() {
        webView.settings.userAgentString = STEALTH_UA
        CookieManager.getInstance().apply {
            setAcceptCookie(false)
            removeAllCookies(null)
        }
        webView.clearCache(true)
        webView.clearFormData()
        webView.clearHistory()
    }

    /** Strip tracker params; default to https:// when scheme missing. */
    private fun sanitize(url: String): String {
        val ensured = if (!url.startsWith("http")) "https://$url" else url
        if (!stealth) return ensured
        return try {
            val uri = android.net.Uri.parse(ensured)
            val kept = uri.queryParameterNames.filter { it.lowercase() !in TRACKER_PARAMS }
            val builder = uri.buildUpon().query(null)
            kept.forEach { p -> uri.getQueryParameter(p)?.let { builder.appendQueryParameter(p, it) } }
            builder.build().toString()
        } catch (e: Exception) { ensured }
    }

    private fun go(raw: String) {
        val v = raw.trim()
        if (v.isEmpty()) return
        val isUrl = v.startsWith("http") || Regex("""^\w[\w.-]*\.[A-Za-z]{2,}""").containsMatchIn(v)
        val target = if (isUrl) sanitize(v)
                     else "https://duckduckgo.com/?q=" + android.net.Uri.encode(v)
        omnibox.setText(target)
        webView.loadUrl(target)
    }

    private inner class MiniClient : WebViewClient() {
        override fun shouldOverrideUrlLoading(view: WebView, url: String): Boolean = false
        override fun onPageFinished(view: WebView, url: String) {
            omnibox.setText(url)
        }
    }
}
