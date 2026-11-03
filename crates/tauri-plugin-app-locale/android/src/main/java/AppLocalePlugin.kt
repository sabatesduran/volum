package dev.didac.volum.applocale

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.util.Locale

@TauriPlugin
class AppLocalePlugin(private val activity: Activity) : Plugin(activity) {
    @Command
    fun getAppLocale(invoke: Invoke) {
        val locales = activity.resources.configuration.locales
        val locale = if (locales.isEmpty) Locale.getDefault() else locales[0]
        invoke.resolve(JSObject().apply { put("locale", locale.toLanguageTag()) })
    }

    @Command
    fun openLanguageSettings(invoke: Invoke) {
        val action = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            Settings.ACTION_APP_LOCALE_SETTINGS
        } else {
            Settings.ACTION_APPLICATION_DETAILS_SETTINGS
        }
        val intent = Intent(action, Uri.fromParts("package", activity.packageName, null))
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        activity.startActivity(intent)
        invoke.resolve()
    }
}
