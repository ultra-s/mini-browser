package dev.mini.browser

import android.Manifest
import android.annotation.SuppressLint
import android.app.Activity
import android.app.AlertDialog
import android.app.DownloadManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Color
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.view.inputmethod.EditorInfo
import android.webkit.CookieManager
import android.webkit.DownloadListener
import android.webkit.PermissionRequest
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.HorizontalScrollView
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import android.widget.Toast

/**
 * Mini for Android — full-featured, fast, clean.
 * Autofit: no hardcoded screen sizes; layout resizes with system bars and keyboard.
 */
class MainActivity : Activity() {

    companion object {
        const val STEALTH_UA = "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/154.0.0.0 Mobile Safari/537.36 MiniStealth/1.0"
        val TRACKER_PARAMS = setOf(
            "utm_source","utm_medium","utm_campaign","utm_term","utm_content","utm_id",
            "fbclid","gclid","dclid","msclkid","twclid","igshid","mc_cid","mc_eid",
            "_hsenc","_hsmi","yclid","ttclid","s_kwcid","li_fat_id")
        const val REQ_WRITE = 101
        const val REQ_WEB_PERMISSION = 102
        const val REQ_NOTIFICATIONS = 103
    }

    private lateinit var root: FrameLayout
    private lateinit var webView: WebView
    private lateinit var omnibox: EditText
    private lateinit var progress: ProgressBar
    private lateinit var securityChip: TextView
    private lateinit var tabStripRow: LinearLayout
    private lateinit var stealthBadge: TextView

    private val tabs = mutableListOf<WebView>()
    private val tabTitles = mutableListOf<String>()
    private var currentTab = -1
    private var stealth = false
    private var desktopMode = false
    private var pendingWebPermission: PermissionRequest? = null
    private var pendingDownloadUrl: String? = null
    private var pendingDownloadDisposition: String? = null
    private var pendingDownloadMime: String? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        stealth = intent.getBooleanExtra("stealth", false)
        window.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE)
        buildUi()
        newTab(initialUrl())
        requestNotificationPermissionIfNeeded()
    }

    private fun initialUrl(): String {
        val target = intent?.dataString
        return when {
            target != null -> sanitize(target)
            else -> "file:///android_asset/start.html" + if (stealth) "?stealth=1" else ""
        }
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        if (!stealth && currentTab >= 0) outState.putString("url", webView.url)
    }

    override fun onBackPressed() {
        if (::webView.isInitialized && webView.canGoBack()) webView.goBack()
        else if (tabs.size > 1) closeTab(currentTab)
        else super.onBackPressed()
    }

    override fun onDestroy() {
        if (stealth) purgeEverything()
        super.onDestroy()
    }

    override fun onPause() {
        super.onPause()
        if (stealth) purgeEverything()
    }

    private fun dp(v: Int) = (v * resources.displayMetrics.density).toInt()

    private fun buildUi() {
        root = FrameLayout(this).apply { setBackgroundColor(Color.parseColor("#0E0D12")) }

        webView = WebView(this)
        root.addView(webView, FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))

        progress = ProgressBar(this, null, android.R.attr.progressBarStyleHorizontal).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, dp(3), Gravity.TOP)
            max = 100
            progressTintList = android.content.res.ColorStateList.valueOf(Color.parseColor("#FF7A45"))
            progressBackgroundTintList = android.content.res.ColorStateList.valueOf(Color.TRANSPARENT)
            visibility = View.GONE
        }
        root.addView(progress)

        val topChrome = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(Color.parseColor("#EE16151C"))
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT, Gravity.TOP)
            setPadding(dp(8), dp(6), dp(8), dp(6))
        }

        val omniboxRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL; gravity = Gravity.CENTER_VERTICAL }

        securityChip = TextView(this).apply {
            text = "🔒"; textSize = 13f; setPadding(dp(6), 0, dp(6), 0) }
        omniboxRow.addView(securityChip)

        omnibox = EditText(this).apply {
            hint = "Search or type URL"
            setSingleLine()
            imeOptions = EditorInfo.IME_ACTION_GO or EditorInfo.IME_FLAG_NO_EXTRACT_UI
            setTextColor(Color.parseColor("#E9E6F0"))
            setHintTextColor(Color.parseColor("#6B6675"))
            setBackgroundColor(Color.parseColor("#1E1C26"))
            setPadding(dp(14), dp(10), dp(14), dp(10))
            layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
            setOnEditorActionListener { _, action, _ ->
                if (action == EditorInfo.IME_ACTION_GO) { go(omnibox.text.toString()); true } else false
            }
        }
        omniboxRow.addView(omnibox)

        val refresh = TextView(this).apply {
            text = "⟳"; textSize = 18f; setPadding(dp(10), 0, dp(4), 0)
            setTextColor(Color.parseColor("#8B8798"))
            setOnClickListener { if (webView.url != null) webView.reload() }
        }
        omniboxRow.addView(refresh)
        topChrome.addView(omniboxRow)

        tabStripRow = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
        val tabStrip = HorizontalScrollView(this).apply {
            isHorizontalScrollBarEnabled = false
            addView(tabStripRow)
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT).apply {
                topMargin = dp(6)
            }
        }
        topChrome.addView(tabStrip)

        stealthBadge = TextView(this).apply {
            text = "● STEALTH"
            setTextColor(Color.parseColor("#B06CFF")); textSize = 10f; letterSpacing = 0.2f
            visibility = if (stealth) View.VISIBLE else View.GONE
            setPadding(0, dp(4), 0, dp(2))
        }
        topChrome.addView(stealthBadge)
        root.addView(topChrome)

        val bottomBar = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER
            setBackgroundColor(Color.parseColor("#EE16151C"))
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, dp(52), Gravity.BOTTOM)
        }
        fun tb(label: String, onTap: () -> Unit) = TextView(this).apply {
            text = label; textSize = 20f; gravity = Gravity.CENTER
            setTextColor(Color.parseColor("#C9C5D4"))
            layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.MATCH_PARENT, 1f)
            setOnClickListener { onTap() }
        }
        bottomBar.addView(tb("‹") { if (webView.canGoBack()) webView.goBack() })
        bottomBar.addView(tb("›") { if (webView.canGoForward()) webView.goForward() })
        bottomBar.addView(tb("⌂") { go("mini://home") })
        bottomBar.addView(tb("⌕") { showFindBar() })
        bottomBar.addView(tb("⋮") { showMenu() })
        root.addView(bottomBar)

        setContentView(root)
    }

    private fun showMenu() {
        val items = listOf(
            "New tab" to { newTab("file:///android_asset/start.html") },
            if (desktopMode) "Mobile site" to { toggleDesktopMode(false) }
            else "Desktop site" to { toggleDesktopMode(true) },
            "Find in page" to { showFindBar() },
            "New stealth tab" to { startStealthTab() },
            "Share" to { sharePage() },
            "Purge data" to { purgeEverything(); Toast.makeText(this, "Data purged", Toast.LENGTH_SHORT).show() }
        )
        val labels = items.map { it.first }.toTypedArray()
        AlertDialog.Builder(this, android.R.style.Theme_Material_Dialog_Alert)
            .setTitle("Mini")
            .setItems(labels) { _, which -> items[which].second() }
            .show()
    }

    private fun showFindBar() {
        val input = EditText(this).apply {
            hint = "Find in page"; setSingleLine()
            setPadding(dp(14), dp(10), dp(14), dp(10)) }
        AlertDialog.Builder(this, android.R.style.Theme_Material_Dialog_Alert)
            .setTitle("Find in page")
            .setView(input)
            .setPositiveButton("Find") { _, _ -> webView.findAllAsync(input.text.toString()) }
            .setNegativeButton("Close") { _, _ -> webView.clearMatches() }
            .show()
    }

    private fun sharePage() {
        val url = webView.url ?: return
        startActivity(Intent.createChooser(Intent(Intent.ACTION_SEND).apply {
            type = "text/plain"; putExtra(Intent.EXTRA_TEXT, url)
        }, "Share via"))
    }

    private fun newTab(url: String) {
        val w = makeWebView()
        tabs.add(w); tabTitles.add("New tab")
        switchToTab(tabs.size - 1)
        w.loadUrl(url)
    }

    private fun startStealthTab() {
        startActivity(Intent(this, MainActivity::class.java).apply {
            putExtra("stealth", true)
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_EXCLUDE_FROM_RECENTS
        })
    }

    private fun switchToTab(index: Int) {
        if (index !in tabs.indices) return
        if (::webView.isInitialized) root.removeView(webView)
        currentTab = index
        val w = tabs[index]
        root.addView(w, 0, FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        webView = w
        omnibox.setText(w.url ?: "")
        refreshTabStrip()
    }

    private fun closeTab(index: Int) {
        if (index !in tabs.indices) return
        val w = tabs[index]
        if (stealth) { w.clearCache(true); w.clearHistory() }
        w.destroy()
        tabs.removeAt(index); tabTitles.removeAt(index)
        when {
            tabs.isEmpty() -> finish()
            currentTab >= tabs.size -> switchToTab(tabs.size - 1)
            currentTab > index -> switchToTab(currentTab - 1)
            else -> switchToTab(currentTab)
        }
    }

    private fun refreshTabStrip() {
        tabStripRow.removeAllViews()
        tabs.forEachIndexed { i, _ ->
            val chip = TextView(this).apply {
                text = ("⧉ " + tabTitles[i]).take(24) + if (i == currentTab) " ●" else ""
                textSize = 12f
                setPadding(dp(12), dp(6), dp(12), dp(6))
                setTextColor(if (i == currentTab) Color.parseColor("#FF7A45") else Color.parseColor("#8B8798"))
                setBackgroundColor(if (i == currentTab) Color.parseColor("#2A2833") else Color.TRANSPARENT)
                layoutParams = LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT).apply {
                    marginEnd = dp(6)
                }
                setOnClickListener { switchToTab(i) }
                setOnLongClickListener { closeTab(i); true }
            }
            tabStripRow.addView(chip)
        }
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun makeWebView(): WebView {
        val w = WebView(this)
        w.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
            databaseEnabled = true
            builtInZoomControls = true
            displayZoomControls = false
            useWideViewPort = true
            loadWithOverviewMode = true
            mediaPlaybackRequiresUserGesture = true
            mixedContentMode = WebSettings.MIXED_CONTENT_NEVER_ALLOW
            allowFileAccess = true
            allowContentAccess = true
            cacheMode = if (stealth) WebSettings.LOAD_NO_CACHE else WebSettings.LOAD_DEFAULT
            javaScriptCanOpenWindowsAutomatically = false
            if (stealth) userAgentString = STEALTH_UA
        }
        w.isFocusableInTouchMode = true
        w.setDownloadListener { url, _, disposition, mime, _ ->
            if (Build.VERSION.SDK_INT >= 29 ||
                checkSelfPermission(Manifest.permission.WRITE_EXTERNAL_STORAGE) == PackageManager.PERMISSION_GRANTED) {
                enqueueDownload(url, disposition, mime)
            } else {
                pendingDownloadUrl = url; pendingDownloadDisposition = disposition; pendingDownloadMime = mime
                requestPermissions(arrayOf(Manifest.permission.WRITE_EXTERNAL_STORAGE), REQ_WRITE)
            }
        }
        w.webViewClient = MiniClient()
        w.webChromeClient = MiniChrome()
        return w
    }

    private fun enqueueDownload(url: String, disposition: String?, mime: String?) {
        val req = DownloadManager.Request(Uri.parse(url)).apply {
            setNotificationVisibility(DownloadManager.Request.VISIBILITY_VISIBLE_NOTIFY_COMPLETED)
            val name = android.webkit.URLUtil.guessFileName(url, disposition, mime)
            setDestinationInExternalPublicDir(Environment.DIRECTORY_DOWNLOADS, name)
            setMimeType(mime ?: "application/octet-stream")
        }
        (getSystemService(Context.DOWNLOAD_SERVICE) as DownloadManager).enqueue(req)
        Toast.makeText(this, "Downloading…", Toast.LENGTH_SHORT).show()
    }

    private inner class MiniClient : WebViewClient() {
        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean = false
        override fun onPageFinished(view: WebView, url: String) {
            omnibox.setText(url)
            securityChip.text = if (url.startsWith("https://") || url.startsWith("file://")) "🔒" else "⚠"
            tabTitles[currentTab] = view.title ?: "Tab"
            refreshTabStrip()
        }
    }

    private inner class MiniChrome : WebChromeClient() {
        override fun onProgressChanged(view: WebView?, newProgress: Int) {
            progress.visibility = if (newProgress in 1..99) View.VISIBLE else View.GONE
            progress.progress = newProgress
        }
        override fun onPermissionRequest(request: PermissionRequest) {
            runOnUiThread {
                pendingWebPermission = request
                val wanted = request.resources
                val needs = mutableListOf<String>()
                if (wanted.contains(PermissionRequest.RESOURCE_VIDEO_CAPTURE)) needs.add(Manifest.permission.CAMERA)
                if (wanted.contains(PermissionRequest.RESOURCE_AUDIO_CAPTURE)) needs.add(Manifest.permission.RECORD_AUDIO)
                if (needs.isEmpty()) { request.deny(); return@runOnUiThread }
                requestPermissions(needs.toTypedArray(), REQ_WEB_PERMISSION)
            }
        }
        override fun onGeolocationPermissionsShowPrompt(origin: String?, callback: android.webkit.GeolocationPermissions.Callback?) {
            if (stealth) callback?.invoke(origin, false, false)
            else AlertDialog.Builder(this@MainActivity, android.R.style.Theme_Material_Dialog_Alert)
                .setTitle("Location")
                .setMessage("Allow $origin to know your location?")
                .setPositiveButton("Allow") { _, _ -> callback?.invoke(origin, true, false) }
                .setNegativeButton("Block") { _, _ -> callback?.invoke(origin, false, false) }
                .show()
        }
    }

    override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<out String>, grantResults: IntArray) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        when (requestCode) {
            REQ_WEB_PERMISSION -> {
                val granted = grantResults.isNotEmpty() && grantResults.all { it == PackageManager.PERMISSION_GRANTED }
                pendingWebPermission?.let { if (granted) it.grant(pendingWebPermission!!.resources) else it.deny() }
                pendingWebPermission = null
            }
            REQ_WRITE -> {
                if (grantResults.firstOrNull() == PackageManager.PERMISSION_GRANTED && pendingDownloadUrl != null) {
                    enqueueDownload(pendingDownloadUrl!!, pendingDownloadDisposition, pendingDownloadMime)
                }
                pendingDownloadUrl = null
            }
        }
    }

    private fun requestNotificationPermissionIfNeeded() {
        if (Build.VERSION.SDK_INT >= 33 &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), REQ_NOTIFICATIONS)
        }
    }

    private fun applyStealth() {
        webView.settings.userAgentString = STEALTH_UA
        CookieManager.getInstance().apply { setAcceptCookie(false); removeAllCookies(null) }
        purgeEverything()
    }

    private fun purgeEverything() {
        if (!::webView.isInitialized) return
        webView.apply { clearCache(true); clearFormData(); clearHistory(); clearSslPreferences() }
        CookieManager.getInstance().apply { removeAllCookies(null); flush() }
    }

    private fun toggleDesktopMode(on: Boolean) {
        desktopMode = on
        webView.settings.userAgentString = if (on)
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/154.0.0.0 Safari/537.36"
        else if (stealth) STEALTH_UA
        else WebSettings.getDefaultUserAgent(this)
        webView.settings.useWideViewPort = on
        webView.reload()
    }

    private fun sanitize(url: String): String {
        val ensured = if (!url.startsWith("http") && !url.startsWith("file://") && !url.startsWith("mini://"))
            "https://$url" else url
        if (!stealth) return ensured
        return try {
            val uri = Uri.parse(ensured)
            val kept = uri.queryParameterNames.filter { it.lowercase() !in TRACKER_PARAMS }
            val builder = uri.buildUpon().query(null)
            kept.forEach { p -> uri.getQueryParameter(p)?.let { builder.appendQueryParameter(p, it) } }
            builder.build().toString()
        } catch (e: Exception) { ensured }
    }

    private fun go(raw: String) {
        val v = raw.trim()
        if (v.isEmpty()) return
        val target = when {
            v == "mini://home" -> "file:///android_asset/start.html" + if (stealth) "?stealth=1" else ""
            v.startsWith("http") || v.startsWith("file://") || v.startsWith("mini://") -> sanitize(v)
            Regex("""^\w[\w.-]*\.[A-Za-z]{2,}""").containsMatchIn(v) -> sanitize(v)
            else -> "https://duckduckgo.com/?q=" + Uri.encode(v)
        }
        omnibox.setText(target)
        webView.loadUrl(target)
    }
}
