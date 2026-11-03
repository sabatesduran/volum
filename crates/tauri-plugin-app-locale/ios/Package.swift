// swift-tools-version:5.3
import PackageDescription

let package = Package(
  name: "tauri-plugin-app-locale",
  platforms: [.iOS(.v13)],
  products: [
    .library(name: "tauri-plugin-app-locale", type: .static, targets: ["tauri-plugin-app-locale"])
  ],
  dependencies: [
    .package(name: "Tauri", path: "../.tauri/tauri-api")
  ],
  targets: [
    .target(name: "tauri-plugin-app-locale", dependencies: [.byName(name: "Tauri")], path: "Sources")
  ]
)
