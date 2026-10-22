# Supported formats and operations

This matrix lists only what is implemented and tested in OmniDrop 0.1.0.

## Images

| Input ↓ / Output → | PNG | JPEG | WebP | AVIF | TIFF | BMP |
| --- | :-: | :-: | :-: | :-: | :-: | :-: |
| PNG / APNG | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| JPEG | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| WebP | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| TIFF | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| BMP | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| GIF | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| ICO | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| AVIF | — | — | — | — | — | — |

| Option | Details |
| --- | --- |
| Presets | JPEG/WebP quality 60 / 80 / 92; AVIF quality 50 / 68 / 82. PNG, TIFF and BMP are lossless (PNG uses stronger compression for "Smallest file"). |
| Manual quality | 1–100 for JPEG, WebP and AVIF. |
| Lossless WebP | Keeps every pixel. |
| Resize | Fit within a width/height (keeps proportions, never enlarges) or scale 1–400%. Lanczos filter. |
| Transparency | Kept for PNG, WebP, AVIF, TIFF and BMP; filled with a chosen colour (white by default) for JPEG. |
| Orientation | EXIF orientation is applied, so photos come out upright. |
| Colour profile | Embedded ICC profiles are kept for PNG, JPEG and TIFF; OmniDrop notes when a profile cannot be kept (WebP, AVIF, BMP). |
| Metadata | Other metadata (EXIF camera data, GPS) is not copied to the result. |
| Limits | Up to 40,000 px per side and 3 GB of decoded memory; WebP output up to 16,383 px per side, JPEG up to 65,535 px. |

**Not supported:** reading AVIF (no AVIF decoder is bundled), HEIC/HEIF, RAW camera formats, SVG.
Animated GIF, APNG and animated WebP are converted from the first frame, with a notice.

## PDF

| Operation | Details |
| --- | --- |
| Merge | Two or more PDFs, in the order of the list (reorder with the arrows). Pages keep their content, size, rotation and inherited resources. |
| Split | One PDF per page, or one per range (`1-3, 5, 8-`). Results go into a folder named after the document. |
| Rotate | 90° clockwise, 180° or 90° counter-clockwise, for all or selected pages. |
| Export pages as images | PNG, JPEG or WebP at 96, 150 or 300 dpi, for all or selected pages. Pages render on white in pure Rust (hayro), with built-in substitutes for the 14 standard PDF fonts. |

**Not preserved when merging or splitting:** bookmarks/outlines, interactive forms, and links that
point to pages of another document. **Not supported:** PDFs that need a password to open. PDFs
protected only by an owner password are processed, and the results are saved without those
permission restrictions. Rendering limitations of hayro: blend modes, knockout
groups and colour-key masking may render imperfectly.

## Audio and video (requires FFmpeg)

OmniDrop uses the FFmpeg installed on the computer. Each output is offered only if that FFmpeg has
the required encoder.

| Output | Encoder(s) | Presets (Smallest / Balanced / Best) |
| --- | --- | --- |
| MP3 | libmp3lame | 128 / 192 / 320 kbps |
| AAC (.m4a) | aac | 128 / 192 / 256 kbps |
| FLAC | flac | lossless (compression level 8 for "Smallest") |
| WAV | pcm_s16le | lossless, 16-bit |
| MP4 | libx264 + aac | CRF 28 / 23 / 18, H.264 yuv420p, fast start |
| MKV | libx264 + aac | CRF 28 / 23 / 18 |
| WebM | libvpx-vp9 + libopus (or libvorbis) | CRF 38 / 32 / 26 |

Inputs: MP3, WAV, FLAC, AAC, M4A, OGG/Opus for audio; MP4, MOV, MKV, WebM and AVI for video (and
for extracting audio from video). Options: maximum resolution (2160p–480p, never enlarges) and
removing the audio track. Subtitles and data streams are not copied. Conversions have a time
limit of at least 30 minutes (20× the media duration, up to 12 hours).

## Archives

| Format | Create | Extract |
| --- | :-: | :-: |
| ZIP (Deflate; reads Deflate64, BZip2, LZMA, XZ) | ✓ | ✓ |
| 7z (LZMA2) | ✓ | ✓ |
| TAR.GZ / TGZ | ✓ | ✓ |
| TAR (no compression) | ✓ | ✓ |

Creating an archive keeps the structure of imported folders. Presets choose the compression level
(9 / 6 / 1; 7z 9 / 5 / 1).

Extraction safety: every archive is extracted into its own new folder. Entries with absolute paths,
`..` components or drive prefixes, symbolic and hard links, and devices are skipped and counted.
Extraction stops — and leaves nothing behind — beyond 200,000 entries, 50 GB, or an expansion
ratio above 500:1 once 256 MB were written. **Not supported:** password-protected archives, RAR.

## Editable documents

DOCX, XLSX, PPTX, ODT, ODS, ODP, DOC, XLS, PPT, RTF and EPUB are recognized and can be compressed
into an archive, but are not converted. Converting editable documents is planned as a separate,
dedicated engine.
