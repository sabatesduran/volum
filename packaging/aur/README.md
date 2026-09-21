# Arch Linux package

Volum publishes the prebuilt x86_64 AppImage as the `volum-bin` AUR package. The release workflow renders `PKGBUILD.template` with the release version and AppImage checksum, validates it with Arch's `makepkg`, attaches the resulting package to the GitHub release, and publishes the source files to the AUR when `AUR_SSH_PRIVATE_KEY` is configured.

The dedicated AUR key must belong to an account that owns `volum-bin`. Add its private half as the `AUR_SSH_PRIVATE_KEY` GitHub Actions secret; never commit it.

To render a package locally after an AppImage has been built:

```bash
python3 packaging/aur/render.py \
  --version 0.1.5 \
  --appimage-sha256 "$(sha256sum Volum_linux_x86_64.AppImage | cut -d' ' -f1)" \
  --output /tmp/volum-aur
```
