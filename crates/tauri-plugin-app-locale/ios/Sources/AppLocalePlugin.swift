import Foundation
import Tauri
import UIKit

final class AppLocalePlugin: Plugin {
  @objc func getAppLocale(_ invoke: Invoke) {
    let locale = Bundle.main.preferredLocalizations.first
      ?? Locale.preferredLanguages.first
      ?? "en"
    invoke.resolve(["locale": locale])
  }

  @objc func openLanguageSettings(_ invoke: Invoke) {
    guard let url = URL(string: UIApplication.openSettingsURLString) else {
      invoke.reject("The app settings URL is unavailable.")
      return
    }
    DispatchQueue.main.async {
      UIApplication.shared.open(url) { _ in invoke.resolve() }
    }
  }
}

@_cdecl("init_plugin_app_locale")
public func initPlugin() -> Plugin {
  AppLocalePlugin()
}
