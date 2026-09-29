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
        versionCode = 22
        versionName = "1.2.1"
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    signingConfigs {
        create("release") {
            val storeFileEnv = System.getenv("MINI_STORE_FILE")
            if (storeFileEnv != null) {
                storeFile = file(storeFileEnv)
                storePassword = System.getenv("MINI_STORE_PASS")
                keyAlias = System.getenv("MINI_KEY_ALIAS")
                keyPassword = System.getenv("MINI_KEY_PASS")
            }
        }
    }
    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            val storeFileEnv = System.getenv("MINI_STORE_FILE")
            signingConfig = if (storeFileEnv != null) signingConfigs.getByName("release") else null
        }
    }
    dependenciesInfo {
        includeInApk = false
    }
}

dependencies {
    implementation("androidx.activity:activity-ktx:1.9.2")
    implementation("androidx.core:core-ktx:1.13.1")
}
