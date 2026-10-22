# Building, testing and releasing

## Prerequisites

| Tool | Version |
| --- | --- |
| Node.js | 22 or newer (CI uses 24) |
| Rust | latest stable (`rustup default stable`) |
| Tauri system dependencies | see below |

**Windows:** Microsoft C++ Build Tools ("Desktop development with C++") and WebView2 (preinstalled on
Windows 10/11).

**macOS:** Xcode Command Line Tools (`xcode-select --install`).

**Debian/Ubuntu:**

```bash
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf build-essential
```

Other distributions: see the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

Optional: **FFmpeg** on the `PATH` enables audio/video in the app and runs the media tests.

## Run

```bash
npm ci
npm run tauri dev
```

`npm run dev` alone serves the interface in a browser with a simulated backend (`src/lib/mock.ts`),
handy for UI work. Add `?demo=files`, `?demo=running` or `?demo=done`, and `&theme=dark` or
`&lang=en`, to jump to a state.

## Test

```bash
cargo test --workspace      # engine and application model
npm test                    # interface (Vitest + Testing Library)
npm run typecheck
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

The engine tests generate every fixture at runtime (images, PDFs, archives, malicious archives,
audio and video through FFmpeg). Media tests are skipped with a message when FFmpeg is missing;
CI installs it on Linux so they always run there.

`cargo test` also regenerates the TypeScript contracts in `src/lib/bindings` (via `ts-rs`). Commit
them; CI fails when they are out of date.

### Manual checks before a release

On a clean machine without development tools (a fresh VM), for each installer:

1. Install, launch, and confirm the window opens with the drop zone.
2. Drop a folder of mixed files; convert images to WebP; open the folder; confirm originals unchanged.
3. Merge two PDFs, split one, export pages as PNG.
4. Create a ZIP and extract it.
5. Without FFmpeg: audio/video shows "Needs FFmpeg"; install FFmpeg, press *Check again*, convert a file.
6. Keyboard only: add files (<kbd>Ctrl</kbd>+<kbd>O</kbd>), choose a format with the arrow keys, start with <kbd>Ctrl</kbd>+<kbd>Enter</kbd>.
7. Switch to the dark theme and to English; restart and confirm the settings persisted.

## Build installers

```bash
npm run tauri build
```

Bundles are written to `target/release/bundle/` (`nsis` and `msi` on Windows, `dmg` and `macos` on
macOS, `deb`, `rpm` and `appimage` on Linux). For a universal macOS build:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run tauri build -- --target universal-apple-darwin
```

## Release

1. Update the version in `package.json`, `Cargo.toml` (`[workspace.package]`) and
   `src-tauri/tauri.conf.json`, and add a section to `CHANGELOG.md`.
2. Commit, then tag and push: `git tag v0.2.0 && git push origin v0.2.0`.
3. The [Release workflow](../.github/workflows/release.yml) creates a draft release with the changelog
   section, builds Windows, Linux and universal macOS installers in parallel, uploads them, and
   publishes the release only after all three builds succeed.

The installers are not code-signed. To sign them, add a Windows certificate and an Apple Developer ID
to the workflow (see the Tauri guides on [Windows](https://v2.tauri.app/distribute/sign/windows/) and
[macOS](https://v2.tauri.app/distribute/sign/macos/) signing).

## Third-party licenses

After changing dependencies, regenerate the inventory:

```bash
node scripts/third-party.mjs
```
