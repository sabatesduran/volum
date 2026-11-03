const COMMANDS: &[&str] = &["get_app_locale", "open_language_settings"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .ios_path("ios")
        .try_build()
        .expect("failed to build app-locale mobile plugin");
}
