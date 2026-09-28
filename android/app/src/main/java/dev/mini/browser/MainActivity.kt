package dev.mini.browser

import android.app.Activity
import android.os.Bundle
import android.webkit.WebView
import android.webkit.WebViewClient

/**
 * M5 Android shell. Renders tabs via the system WebView first (minimal, ships today);
 * GeckoView variant tracked in ANDROID_ROADMAP.md M5->M6.
 * Tab state lives in mini-core (Rust), bridged in at M6 via cargo-ndk JNI.
 */
class MainActivity : Activity() {
    private lateinit var web: WebView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        web = WebView(this)
        web.settings.javaScriptEnabled = true
        web.webViewClient = WebViewClient()
        setContentView(web)

        intent?.dataString?.let { web.loadUrl(it) } ?: web.loadUrl("https://duckduckgo.com/")
    }

    override fun onBackPressed() {
        if (web.canGoBack()) web.goBack() else super.onBackPressed()
    }
}
