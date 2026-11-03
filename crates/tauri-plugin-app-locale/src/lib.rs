#![cfg(mobile)]

use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime,
};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "dev.didac.volum.applocale";

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_app_locale);

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("app-locale")
        .setup(|_app, api| {
            #[cfg(target_os = "android")]
            api.register_android_plugin(PLUGIN_IDENTIFIER, "AppLocalePlugin")?;
            #[cfg(target_os = "ios")]
            api.register_ios_plugin(init_plugin_app_locale)?;
            Ok(())
        })
        .build()
}
