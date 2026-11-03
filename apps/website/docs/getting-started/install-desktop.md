---
title: Install Volum Desktop
description: Install Volum on macOS, Windows, or Linux.
---

# Install Volum Desktop

Download the current release from [GitHub Releases](https://github.com/sabatesduran/volum/releases/latest). Volum does not require an account or activation key.

## macOS

1. Download the universal `.dmg`.
2. Open the disk image.
3. Drag Volum into **Applications**.
4. Open Volum from Applications.

The macOS release is signed and notarized and supports both Apple silicon and Intel Macs.

## Windows

1. Choose the x64 installer for most Windows computers.
2. Choose the ARM64 installer for native Windows on Arm devices.
3. Run the `.exe` installer.
4. Start Volum from the Start menu.

::: info SmartScreen
Windows installers are not yet Authenticode-signed. Microsoft Defender SmartScreen may ask you to confirm that you want to run the installer. Release checksum and Tauri updater signature files are published alongside the installer.
:::

## Linux

1. Download the x86_64 AppImage.
2. Make it executable.
3. Run it from your file manager or terminal.

```bash
chmod +x Volum_linux_x86_64.AppImage
./Volum_linux_x86_64.AppImage
```

An Arch Linux package is also prepared. Check the release notes for its current availability.

## Updates

Volum checks for signed Desktop updates. You decide when to download and install them. You can also install a newer release manually over the existing application; your database remains in the platform app-data directory.

Before a major upgrade, keep a current Volum backup and a separate backup of your original model folders.

## Next step

Open Volum and [create your first library](./first-library).
