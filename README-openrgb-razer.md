# openrgb-razer

[OpenRGB](https://openrgb.org) plus one fix: **the Razer Huntsman V3 Pro TKL (USB `1532:02A7`, black and white) works on Linux.**

This branch is part of [UnsyncLabs/Chroma-Desk](https://github.com/UnsyncLabs/Chroma-Desk) (the [chroma-desk](https://github.com/UnsyncLabs/Chroma-Desk) UI talks to an OpenRGB server). The code is OpenRGB's, licensed GPL-2.0-or-later; the fix is small and meant to be sent upstream.

## What it fixes

Interface 3 of the Huntsman V3 Pro TKL, the one that takes the lighting commands, has no interrupt IN endpoint. The Linux `usbhid` driver refuses to bind it (`couldn't find an input interrupt endpoint`), so no `/dev/hidraw*` node exists and the official OpenRGB never sees the keyboard. openrgb-razer registers an extra detector that opens that interface with hidapi's **libusb** backend (feature reports only need control transfers). The existing hidraw detectors are untouched.

Not affected: other Razer devices (they keep working through hidraw as in the official build), and Windows/macOS, where the interface is a regular HID device and the official OpenRGB already works. **The fix is Linux-only.**

## Do you need it?

- You own a Razer Huntsman V3 Pro TKL on Linux: **yes**.
- Any other device: use the official OpenRGB. This build behaves the same for everything else.

`install.sh` from chroma-desk detects Razer hardware (`lsusb` / `/sys` vendor `1532`) and asks which one to install.

## Relationship with the official OpenRGB

openrgb-razer **replaces** the official package: it installs the same files (`/usr/bin/openrgb`, udev rules, desktop entry) and declares `Provides/Conflicts/Replaces: openrgb` (Arch, deb) or `Provides/Obsoletes: openrgb` (rpm), so chroma-desk and other packages that depend on `openrgb` stay satisfied. To go back, remove it and install the official package.

## Branches

| Branch | What it is |
| --- | --- |
| `openrgb-razer` | OpenRGB **release_1.1** + the fix. Stable; releases are tagged `openrgb-razer-v1.1-r1`, `-r2`... |
| `openrgb-razer-master` | The fix rebased on the current OpenRGB `master`. Kept up to date by the `sync-openrgb` workflow; releases are pre-releases tagged `openrgb-razer-master-YYYYMMDD`. If a rebase needs manual work, the workflow opens an issue. |
| `electron-openrgb` | An unmodified copy of OpenRGB `master` (kept in sync automatically). |

## Downloads

Every [release](https://github.com/UnsyncLabs/Chroma-Desk/releases) tagged `openrgb-razer-v*` has, for x86_64 and aarch64 (Arch: x86_64 only):

| File | For |
| --- | --- |
| `openrgb-razer_<ver>_{amd64,arm64}.deb` | Debian 13 (trixie) |
| `openrgb-razer-<ver>-<rel>.fc44.{x86_64,aarch64}.rpm` | Fedora 44 |
| `openrgb-razer-<ver>-<rel>-x86_64.pkg.tar.zst` | Arch Linux and derivatives |
| `openrgb-razer-<ver>-{x86_64,aarch64}.AppImage` | any other distribution (e.g. Ubuntu); needs the udev rules below |
| `60-openrgb.rules` | udev rules for the AppImage route (`/etc/udev/rules.d/`) |
| `SHA256SUMS` | checksums of all of the above |

Install (Arch example): `sudo pacman -U openrgb-razer-1.1-1-x86_64.pkg.tar.zst`. After installing, reload udev (`sudo udevadm control --reload-rules && sudo udevadm trigger`) and unplug/replug the keyboard, or just run chroma-desk's `install.sh`.

## Build it yourself

```sh
# Arch
cd packaging/arch && makepkg -si
# anywhere (Qt 6, libusb, hidapi, mbedtls 3.x installed)
mkdir build && cd build && qmake6 ../OpenRGB.pro PREFIX=/usr && make -j"$(nproc)"
```

The release workflow (`openrgb-razer-release.yml`, kept on the `main` branch so automation can rebase these branches freely) builds the AppImage with OpenRGB's own `scripts/build-appimage.sh` and OpenRGB's CI image, the `.deb` in `debian:trixie`, the `.rpm` in `fedora:44` and the Arch package in `archlinux`, from the files in `packaging/`.

## Upstream

OpenRGB: <https://gitlab.com/CalcProgrammer1/OpenRGB>. Please report problems unrelated to the Huntsman V3 Pro TKL fix there.
