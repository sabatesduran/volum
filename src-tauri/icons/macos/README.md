# macOS app icon

`AppIcon.icon` is the macOS 26+ Icon Composer source. `Assets.car` is its compiled catalog and is committed so release builds do not require Icon Composer.

Regenerate the catalog on macOS with Xcode 26 or newer:

```bash
output="$(mktemp -d)"
xcrun actool src-tauri/icons/macos/AppIcon.icon \
  --compile "$output" \
  --output-partial-info-plist "$output/partial.plist" \
  --app-icon AppIcon \
  --include-all-app-icons \
  --enable-on-demand-resources NO \
  --development-region en \
  --target-device mac \
  --minimum-deployment-target 10.13 \
  --platform macosx
cp "$output/Assets.car" src-tauri/icons/macos/Assets.car
rm -rf "$output"
```

The existing `icons/icon.icns` remains the fallback for macOS 25 and older.
