---
name: release-readiness
description: Steps to ship an OmniDrop release — Windows, macOS and Linux builds, native dependencies, licenses, installers and clean-machine tests. Use before tagging a version.
---

# Release readiness

## Checklist

1. **Green CI** on `main` (frontend, plus Rust on ubuntu-22.04, windows-latest and macos-latest).
2. **Version bumped** in `package.json`, `Cargo.toml` (`[workspace.package]`) and `src-tauri/tauri.conf.json`; `Cargo.lock` updated (`cargo check`).
3. **CHANGELOG.md** has `## [x.y.z] - YYYY-MM-DD`; the release workflow copies this section into the release notes.
4. **Licenses:** run `node scripts/third-party.mjs` and review new licenses (no GPL-only code in the binaries; FFmpeg is never bundled).
5. **Docs current:** README matrix, `docs/FORMATS.md` limitations, screenshots taken from the real app (no mockups).
6. **Tag and push:** `git tag vX.Y.Z && git push origin vX.Y.Z`. The workflow creates a draft, builds NSIS and MSI (Windows), DEB, RPM and AppImage (Linux, built on Ubuntu 22.04 for an older glibc baseline) and a universal DMG (macOS), then publishes.
7. **Verify the release page** lists every asset: `*_x64-setup.exe`, `*_x64_en-US.msi`, `*_universal.dmg`, `*.app.tar.gz`, `*_amd64.deb`, `*.x86_64.rpm`, `*_amd64.AppImage`.

## Clean-machine test (per OS, a VM without development tools)

- Install from the release asset; the app starts and the drop zone is visible.
- Convert images, merge and split a PDF, create and extract a ZIP.
- Without FFmpeg, audio and video show "Needs FFmpeg"; after installing it, "Check again" enables them.
- Uninstalling leaves the user's files untouched.

## Platform notes

- Windows: unsigned, so SmartScreen shows "More info → Run anyway". The installer downloads WebView2 if it is missing.
- macOS: ad-hoc signed (`signingIdentity: "-"`), not notarized: right-click → Open, or `xattr -dr com.apple.quarantine`.
- Linux: the AppImage needs FUSE 2 on some distributions (`libfuse2`).

## Verifiable criteria

- [ ] The release workflow succeeded on all three platforms.
- [ ] The release is published (not a draft) and marked as latest.
- [ ] Each installer passed the clean-machine test.
