# Changelog

All notable changes to OmniDrop are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-10-22

The first public release.

### Added

- Drag-and-drop desktop app for Windows, macOS and Linux, built with Tauri 2, React and Rust.
- Image conversion between PNG, JPEG, WebP, TIFF, BMP, GIF and ICO inputs and PNG, JPEG, WebP, AVIF, TIFF and BMP outputs, with "Smallest file / Balanced / Best quality" presets, manual quality, lossless WebP, resizing (fit or percentage) and a background colour for transparency.
- PDF tools: merge (in list order), split by page or by ranges, rotate pages, and export pages as PNG, JPEG or WebP images. PDF rendering is done in pure Rust (hayro); no external programs are needed.
- Audio and video conversion through the FFmpeg already installed on the computer: MP3, AAC (M4A), FLAC and WAV audio (also extracted from videos), and MP4, WebM and MKV video with optional resolution limit and audio removal. Formats are offered only when the local FFmpeg has the matching encoder.
- Archives: create ZIP, 7z, TAR.GZ and TAR; extract ZIP, 7z, TAR and TAR.GZ into a folder.
- Batch processing with per-file and overall progress, cancellation of the whole job or of single items, and isolated failures.
- Real format detection from file content, with a warning when the extension does not match.
- Safe output handling: results are written to temporary files, validated, then published; originals are never replaced; name collisions follow a clear rule (keep both, replace, or skip).
- Protection against malicious archives: unsafe paths and links are skipped, and size, entry-count and expansion-ratio limits stop decompression bombs.
- Folder import that keeps the folder structure in the output.
- Recent jobs, light and dark themes, Portuguese (Brazil) and English, keyboard shortcuts and screen-reader support.

[0.1.0]: https://github.com/Ernani1234/Omnidrop/releases/tag/v0.1.0
