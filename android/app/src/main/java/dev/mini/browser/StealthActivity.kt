package dev.mini.browser

import android.app.Activity
import android.content.Intent
import android.os.Bundle

/** Launcher for stealth tabs: separate task, excluded from recents. */
class StealthActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        startActivity(Intent(this, MainActivity::class.java).apply {
            putExtra("stealth", true)
            data = intent?.data
        })
        finish()
    }
}
