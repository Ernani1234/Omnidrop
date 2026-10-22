# 0002 — Pure Rust engines for images, PDF and archives

## Context

The brief suggested libvips or ImageMagick for images and "dedicated tools" for PDF and archives,
asking to weigh size, licensing, platform support and packaging. Shipping native binaries for three
platforms (plus macOS universal builds) adds packaging risk and license obligations.

## Decision

- Images: the `image` crate for decoding/encoding, libwebp (compiled in through `libwebp-sys`) for
  lossy WebP, `ravif`/rav1e for AVIF.
- PDF: `lopdf` for merge/split/rotate and `hayro` (pure Rust rasterizer) to render pages.
- Archives: `zip`, `tar` + `flate2`, `sevenz-rust2`.

## Consequences

- No external programs or dynamic libraries to ship; everything builds with Cargo on every platform.
- All licenses are permissive (see THIRD_PARTY.md).
- Known gaps are declared, not hidden: AVIF cannot be read (no pure-Rust AV1 decoder is mature
  enough), HEIC is unsupported, hayro has rendering limitations for some advanced PDF features.
- Swapping an engine later only touches its executor module.
