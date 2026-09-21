#!/usr/bin/env python3
"""Render the release-specific volum-bin AUR source package."""

from __future__ import annotations

import argparse
import hashlib
import re
import shutil
from pathlib import Path


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--appimage-sha256", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()

    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:[._+-][0-9A-Za-z.-]+)?", args.version):
        parser.error("version must be a package-compatible semantic version")
    if not re.fullmatch(r"[0-9a-f]{64}", args.appimage_sha256):
        parser.error("AppImage SHA-256 must be 64 lowercase hexadecimal characters")

    package_dir = Path(__file__).resolve().parent
    repository = package_dir.parents[1]
    desktop = package_dir / "volum.desktop"
    icon = repository / "src-tauri" / "icons" / "128x128.png"
    template = (package_dir / "PKGBUILD.template").read_text()
    replacements = {
        "@VERSION@": args.version,
        "@APPIMAGE_SHA256@": args.appimage_sha256,
        "@DESKTOP_SHA256@": sha256(desktop),
        "@ICON_SHA256@": sha256(icon),
    }
    for placeholder, value in replacements.items():
        template = template.replace(placeholder, value)
    if re.search(r"@[A-Z0-9_]+@", template):
        raise RuntimeError("unresolved PKGBUILD placeholder")

    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / "PKGBUILD").write_text(template)
    shutil.copy2(desktop, args.output / "volum.desktop")
    shutil.copy2(icon, args.output / "volum.png")


if __name__ == "__main__":
    main()
