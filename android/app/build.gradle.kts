plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "dev.mini.browser"
    compileSdk = 35
    defaultConfig {
        applicationId = "dev.mini.browser"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }
    buildTypes {
        release { isMinifyEnabled = true }
    }
}

dependencies {
    // M5: GeckoView shell (Firefox engine). Swap to "system" WebView variant if needed.
    implementation("org.mozilla.geckoview:geckoview:130.0.20240909222904")
    implementation("androidx.activity:activity-ktx:1.9.2")
    implementation("androidx.core:core-ktx:1.13.1")
}
