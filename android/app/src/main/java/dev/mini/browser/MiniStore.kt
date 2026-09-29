package dev.mini.browser

import android.content.Context
import android.content.SharedPreferences
import org.json.JSONArray
import org.json.JSONObject
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * MiniStore — user data persistence. Bookmarks, history, reading list, settings.
 * Stealth mode never writes here.
 */
class MiniStore(context: Context) {

    private val prefs: SharedPreferences =
        context.getSharedPreferences("mini_store", Context.MODE_PRIVATE)

    // ------------------------------------------------------------- bookmarks

    fun bookmarks(): JSONArray = JSONArray(prefs.getString("bookmarks", "[]"))

    fun addBookmark(title: String, url: String) {
        if (url.startsWith("file://")) return
        val arr = bookmarks()
        // dedupe by url
        val filtered = JSONArray()
        for (i in 0 until arr.length()) {
            val o = arr.getJSONObject(i)
            if (o.optString("url") != url) filtered.put(o)
        }
        filtered.put(JSONObject().put("title", title).put("url", url)
            .put("saved", System.currentTimeMillis()))
        prefs.edit().putString("bookmarks", filtered.toString()).apply()
    }

    fun removeBookmark(url: String) {
        val arr = bookmarks()
        val filtered = JSONArray()
        for (i in 0 until arr.length()) {
            if (arr.getJSONObject(i).optString("url") != url) filtered.put(arr.getJSONObject(i))
        }
        prefs.edit().putString("bookmarks", filtered.toString()).apply()
    }

    fun isBookmarked(url: String): Boolean {
        val arr = bookmarks()
        for (i in 0 until arr.length()) {
            if (arr.getJSONObject(i).optString("url") == url) return true
        }
        return false
    }

    // --------------------------------------------------------------- history

    fun history(): JSONArray = JSONArray(prefs.getString("history", "[]"))

    fun addHistory(title: String, url: String) {
        if (url.startsWith("file://")) return
        val arr = history()
        arr.put(JSONObject().put("title", title).put("url", url)
            .put("at", System.currentTimeMillis()))
        // cap at 2000 entries
        while (arr.length() > 2000) arr.remove(0)
        prefs.edit().putString("history", arr.toString()).apply()
    }

    fun clearHistory() { prefs.edit().remove("history").apply() }

    // ---------------------------------------------------------- reading list

    fun readingList(): JSONArray = JSONArray(prefs.getString("reading", "[]"))

    fun addReading(title: String, url: String) {
        if (url.startsWith("file://")) return
        val arr = readingList()
        arr.put(JSONObject().put("title", title).put("url", url))
        prefs.edit().putString("reading", arr.toString()).apply()
    }

    fun removeReading(url: String) {
        val arr = readingList()
        val filtered = JSONArray()
        for (i in 0 until arr.length()) {
            if (arr.getJSONObject(i).optString("url") != url) filtered.put(arr.getJSONObject(i))
        }
        prefs.edit().putString("reading", filtered.toString()).apply()
    }

    // ------------------------------------------------------------- session

    fun saveSession(urls: List<String>, index: Int) {
        val arr = JSONArray()
        urls.forEach { arr.put(it) }
        prefs.edit().putString("session", arr.toString())
            .putInt("session_index", index).apply()
    }

    fun restoreSession(): Pair<List<String>, Int>? {
        val s = prefs.getString("session", null) ?: return null
        val arr = JSONArray(s)
        val urls = (0 until arr.length()).map { arr.getString(it) }
        if (urls.isEmpty()) return null
        return urls to prefs.getInt("session_index", 0).coerceIn(0, urls.size - 1)
    }

    // ------------------------------------------------------------- settings

    /** Custom UA profile: "auto" | "desktop" | "mobile" | "stealth" | literal string. */
    var uaProfile: String
        get() = prefs.getString("ua", "auto")!!
        set(v) = prefs.edit().putString("ua", v).apply()

    fun uaFor(profile: String, systemDefault: String): String = when (profile) {
        "desktop" -> "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/154.0.0.0 Safari/537.36"
        "mobile" -> systemDefault
        "stealth" -> "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/154.0.0.0 Safari/537.36"
        "auto" -> systemDefault
        else -> profile
    }

    var homePage: String
        get() = prefs.getString("home", "file:///android_asset/start.html")!!
        set(v) = prefs.edit().putString("home", v).apply()

    var searchEngine: String   // "ddg" | "google" | "bing" | "brave"
        get() = prefs.getString("engine", "ddg")!!
        set(v) = prefs.edit().putString("engine", v).apply()

    var blockImages: Boolean
        get() = prefs.getBoolean("noimages", false)
        set(v) = prefs.edit().putBoolean("noimages", v).apply()

    var acceptCookies: Boolean
        get() = prefs.getBoolean("cookies", true)
        set(v) = prefs.edit().putBoolean("cookies", v).apply()

    fun searchUrlFor(query: String): String = when (searchEngine) {
        "google" -> "https://www.google.com/search?q="
        "bing" -> "https://www.bing.com/search?q="
        "brave" -> "https://search.brave.com/search?q="
        else -> "https://duckduckgo.com/?q="
    } + android.net.Uri.encode(query)

    // -------------------------------------------------------- data wipe

    fun wipeAll() {
        prefs.edit().clear().apply()
    }

    fun exportJson(): String {
        val o = JSONObject()
            .put("bookmarks", bookmarks())
            .put("history", history())
            .put("reading", readingList())
            .put("engine", searchEngine)
        return o.toString(2)
    }

    fun importJson(json: String): Boolean = try {
        val o = JSONObject(json)
        if (o.has("bookmarks")) prefs.edit().putString("bookmarks", o.getString("bookmarks")).apply()
        if (o.has("reading")) prefs.edit().putString("reading", o.getString("reading")).apply()
        if (o.has("engine")) searchEngine = o.getString("engine")
        true
    } catch (e: Exception) { false }

    companion object {
        fun fmtTime(ms: Long): String =
            SimpleDateFormat("MMM d, HH:mm", Locale.getDefault()).format(Date(ms))
    }
}
