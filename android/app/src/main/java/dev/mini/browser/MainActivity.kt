package dev.mini.browser

import android.Manifest
import android.annotation.SuppressLint
import android.app.Activity
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
import android.widget.TextView

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
    private lateinit var progress: MiniProgress
    private lateinit var securityChip: MiniIconButton
    private lateinit var tabStripRow: LinearLayout
    private lateinit var stealthBadge: MiniIconButton

    private val tabs = mutableListOf<WebView>()
    private val tabTitles = mutableListOf<String>()
    private var currentTab = -1
    private var stealth = false
    private var desktopMode = false
    private var pendingWebPermission: PermissionRequest? = null
    private lateinit var store: MiniStore
    private lateinit var sheet: MiniSheet
    private lateinit var toast: MiniToast
    private var pendingDownloadUrl: String? = null
    private var pendingDownloadDisposition: String? = null
    private var pendingDownloadMime: String? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        stealth = intent.getBooleanExtra("stealth", false)
        store = MiniStore(this)
        sheet = MiniSheet(this)
        (window.decorView as ViewGroup).post { (window.decorView as ViewGroup).addView(sheet, ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT) }
        toast = MiniToast(this)
        window.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE)
        buildUi()
        restoreOrHome()
        requestNotificationPermissionIfNeeded()
    }

    private fun initialUrl(): String {
        val target = intent?.dataString
        return when {
            target != null -> sanitize(target)
            else -> "file:///android_asset/start.html" + if (stealth) "?stealth=1" else ""
        }
    }

    private fun restoreOrHome() {
        val target = intent?.dataString
        if (target != null) { newTab(sanitize(target)); return }
        if (!stealth) {
            store.restoreSession()?.let { (urls, idx) ->
                urls.forEachIndexed { i, u -> newTab(u) }
                // newTab switches to each; land on the saved active one
                if (idx in tabs.indices) switchToTab(idx)
                return
            }
        }
        newTab(store.homePage + if (stealth) "?stealth=1" else "")
    }

    private fun saveSessionNow() {
        if (stealth || tabs.isEmpty()) return
        store.saveSession(tabs.map { it.url ?: "" }, currentTab)
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
        saveSessionNow()
        if (stealth) purgeEverything()
    }

    private fun dp(v: Int) = (v * resources.displayMetrics.density).toInt()

    private fun buildUi() {
        root = FrameLayout(this).apply { setBackgroundColor(Color.parseColor("#0E0D12")) }

        webView = WebView(this)
        root.addView(webView, FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))

        progress = MiniProgress(this).apply {
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

        securityChip = MiniIconButton(this, "🔒", 38f) { showSiteInfo() }
        omniboxRow.addView(securityChip)

        omnibox = EditText(this).apply {
            hint = "Search or type URL"
            setSingleLine()
            imeOptions = EditorInfo.IME_ACTION_GO or EditorInfo.IME_FLAG_NO_EXTRACT_UI
            setTextColor(MiniUi.TEXT)
            setHintTextColor(MiniUi.DIM)
            background = omniboxBg(false)
            setPadding(dp(18), dp(13), dp(18), dp(13))
            textSize = 15f
            layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
            setOnFocusChangeListener { _, has -> background = omniboxBg(has) }
            addTextChangedListener(object : android.text.TextWatcher {
                override fun beforeTextChanged(s: CharSequence?, st: Int, c: Int, a: Int) {}
                override fun onTextChanged(s: CharSequence?, st: Int, b: Int, c: Int) {
                    if (findMode) webView.findAllAsync(s?.toString() ?: "")
                }
                override fun afterTextChanged(s: android.text.Editable?) {}
            })
            setOnEditorActionListener { _, action, _ ->
                if (action == EditorInfo.IME_ACTION_GO) {
                    if (findMode) endFind() else go(omnibox.text.toString())
                    true
                } else false
            }
        }
        omniboxRow.addView(omnibox)

        val refresh = MiniIconButton(this, "⟳", 40f) { if (webView.url != null) webView.reload() }
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
            text = "● S T E A L T H"
            setTextColor(MiniUi.GLOW_B); textSize = 10f; letterSpacing = 0.25f
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
        fun tb(label: String, onTap: () -> Unit) = MiniIconButton(this, label, 48f) { onTap() }.apply {
            layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.MATCH_PARENT, 1f)
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
        val url = webView.url ?: ""
        val title = webView.title ?: url
        val bm = if (url.isNotEmpty() && store.isBookmarked(url)) "Remove bookmark" else "Add bookmark"
        val items = listOf(
            bm to { if (store.isBookmarked(url)) store.removeBookmark(url) else store.addBookmark(title, url) },
            "Save to reading list" to { store.addReading(title, url); toast.show("Saved") },
            "Bookmarks" to { showList("Bookmarks") { store.bookmarks() } },
            "History" to { if (stealth) toast.show("No history in stealth") else showList("History") { store.history() } },
            "Reading list" to { showList("Reading list") { store.readingList() } },
            "Search engine: " + store.searchEngine.uppercase() to { cycleEngine() },
            "Export data" to { exportData() },
            "Wipe all data" to { store.wipeAll(); purgeEverything(); toast.show("All data wiped") },
            "New tab" to { newTab(store.homePage) },
            if (desktopMode) "Mobile site" to { toggleDesktopMode(false) }
            else "Desktop site" to { toggleDesktopMode(true) },
            "Find in page" to { showFindBar() },
            "New stealth tab" to { startStealthTab() },
            "Share" to { sharePage() },
            "Purge data" to { purgeEverything(); toast.show("Data purged") }
        )
        sheet.show("Mini", items.map { (label, act) -> MiniSheet.Row(iconFor(label), label) { act() } })
    }

    private fun showSiteInfo() {
        val url = webView.url ?: return
        val secure = url.startsWith("https://")
        sheet.show("Site", listOf(
            MiniSheet.Row(if (secure) "🔒" else "⚠", url.take(40)) { omnibox.setTextExternal(url); omnibox.requestFocus(); omnibox.showKeyboard() },
            MiniSheet.Row("★", if (store.isBookmarked(url)) "Remove bookmark" else "Add bookmark") {
                if (store.isBookmarked(url)) store.removeBookmark(url) else store.addBookmark(webView.title ?: url, url)
                toast.show("Done")
            },
            MiniSheet.Row("⧉", "Copy link") {
                val cm = getSystemService(Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
                cm.setPrimaryClip(android.content.ClipData.newPlainText("url", url))
                toast.show("Copied")
            }
        ))
    }

    private fun omniboxBg(focused: Boolean): android.graphics.drawable.GradientDrawable {
        val d = android.graphics.drawable.GradientDrawable()
        d.cornerRadius = dp(23f)
        if (focused) {
            d.setColor(0xFF14131B.toInt())
            d.setStroke(dp(1.6f).toInt(), MiniUi.GLOW_A)
        } else {
            d.setColor(0xFF1E1C27.toInt())
            d.setStroke(dp(1f).toInt(), 0x22FFFFFF)
        }
        return d
    }

    private fun iconFor(label: String): String = when {
        label.contains("bookmark", true) -> "★"
        label.contains("reading", true) -> "≡"
        label.contains("history", true) -> "⟲"
        label.contains("engine", true) -> "⌕"
        label.contains("export", true) -> "↥"
        label.contains("wipe", true) || label.contains("purge", true) -> "⌫"
        label.contains("new tab", true) -> "＋"
        label.contains("stealth", true) -> "◈"
        label.contains("desktop", true) -> "▣"
        label.contains("find", true) -> "⌕"
        label.contains("share", true) -> "↗"
        label.contains("download", true) -> "↓"
        else -> "◆"
    }

    private var findMode = false
    private fun showFindBar() {
        findMode = true
        omnibox.setTextExternal("")
        omnibox.hint = "Find in page"
        omnibox.requestFocus()
        omnibox.showKeyboard()
    }

    private fun endFind() {
        findMode = false
        omnibox.hint = "Search or enter address"
        webView.clearMatches()
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
                setTextColor(if (i == currentTab) MiniUi.GLOW_A else MiniUi.DIM)
                background = MiniUi.rounded(this@MainActivity, if (i == currentTab) MiniUi.INK2 else Color.TRANSPARENT, 16f)
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
        toast.show("Downloading…")
    }

    private fun showList(title: String, source: () -> org.json.JSONArray) {
        val arr = source()
        if (arr.length() == 0) { toast.show("Empty"); return }
        val entries = (0 until arr.length()).map { arr.getJSONObject(it) }.reversed().take(50)
        sheet.show(title, entries.map { e ->
            val u = e.optString("url")
            MiniSheet.Row("◇", (e.optString("title").ifEmpty { u }).take(44)) { if (u.isNotEmpty()) go(u) }
        })
    }

    private fun cycleEngine() {
        val engines = listOf("ddg", "google", "bing", "brave")
        val next = engines[(engines.indexOf(store.searchEngine) + 1) % engines.size]
        store.searchEngine = next
        toast.show("Search: $next")
    }

    private fun exportData() {
        try {
            val dir = Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS)
            dir.mkdirs()
            java.io.File(dir, "mini-export.json").writeText(store.exportJson())
            toast.show("Exported to Downloads/mini-export.json")
        } catch (e: Exception) {
            toast.show("Export failed")
        }
    }

    private inner class MiniClient : WebViewClient() {
        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean = false
        override fun onPageFinished(view: WebView, url: String) {
            omnibox.setText(url)
            securityChip.glyph = if (url.startsWith("https://") || url.startsWith("file://")) "🔒" else "⚠"
            securityChip.invalidate()
            tabTitles[currentTab] = view.title ?: "Tab"
            refreshTabStrip()
            if (!stealth && url.startsWith("http")) store.addHistory(view.title ?: url, url)
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
            if (stealth) { callback?.invoke(origin, false, false); return }
            val o = origin ?: ""
            runOnUiThread {
                sheet.show("Location", listOf(
                    MiniSheet.Row("✓", "Allow $o to know your location".take(44)) { callback?.invoke(o, true, false) },
                    MiniSheet.Row("✕", "Block") { callback?.invoke(o, false, false) }
                ))
            }
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
            v == "mini://home" -> store.homePage + if (stealth) "?stealth=1" else ""
            v.startsWith("http") || v.startsWith("file://") || v.startsWith("mini://") -> sanitize(v)
            Regex("""^\w[\w.-]*\.[A-Za-z]{2,}""").containsMatchIn(v) -> sanitize(v)
            else -> store.searchUrlFor(v)
        }
        omnibox.setText(target)
        webView.loadUrl(target)
    }
}
