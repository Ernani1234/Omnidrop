---
name: conversion-testing
description: How to test OmniDrop conversions — reference fixtures, corrupt files, incompatibilities, collisions, cancellation and large batches. Use when writing or reviewing engine or UI tests.
---

# Conversion testing

## Where

- Engine integration tests: `crates/omnidrop-engine/tests/{images,pdf,archives,media,reliability}.rs`, helpers in `tests/common/mod.rs`.
- Application model: unit tests in `crates/omnidrop-app`.
- UI: `src/test/*.test.ts(x)` with `createFakeBackend()` from `src/test/fakeBackend.ts`.

## Fixtures

Generate everything at runtime; never commit binary fixtures.

- Images: `write_png`, `write_image(path, ImageFormat, w, h)` (ICO must be RGBA).
- PDFs: `write_pdf(path, pages)`; page *n* has width `200 + 10n`, so order is checkable with `pdf_page_widths`.
- Archives: `zip::ZipWriter` and `tar::Builder`, including malicious names and symlinks.
- Media: `ffmpeg -f lavfi` (`sine`, `testsrc`); media tests return early with a message when FFmpeg is missing.

## Required cases for any operation

| Case | Example |
| --- | --- |
| Valid conversion for each declared input and output | `every_declared_input_converts_to_every_output` |
| Corrupt file, misleading extension, unknown format | `corrupt_file_fails_alone_and_batch_continues`, `misleading_extension_is_converted_by_content` |
| Destination without permission, full disk (when testable) | `read_only_destination_reports_actionable_error` (Unix) |
| Name collisions and overwrite attempts | `collisions_follow_policy_and_never_touch_existing_files` |
| Cancellation before and during execution | `cancelling_before_start_processes_nothing`, `cancelling_during_a_batch_stops_remaining_items_cleanly` |
| Large batch and isolated failure | `large_batch_completes` (200 files) |
| Readable output, metadata per contract | validator expectations plus decoding the result |

Always assert that originals are byte-identical afterwards and that `no_temp_files(dir)` holds.

## Running

```bash
cargo test --workspace            # add -- --nocapture to see skipped media tests
cargo test -p omnidrop-engine --test archives
npm test
```

## Verifiable criteria

- [ ] A new operation has success, failure, cancellation and collision tests.
- [ ] Tests pass on Linux with FFmpeg (CI) and skip gracefully without it.
