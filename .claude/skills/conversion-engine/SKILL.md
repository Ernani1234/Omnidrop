---
name: conversion-engine
description: Contracts and rules for crates/omnidrop-engine — executors, format validation, process control, resource limits and preserving originals. Use when adding formats or operations, or touching inspection, planning, execution, validation or output.
---

# Conversion engine

Pipeline: `inspector` → `capability` → `plan` → `job` → `executors` → `validator` → `output`.

## Executor contract

```rust
pub fn convert(options: &XOptions, task: &TaskSpec, ctx: &TaskContext<'_>) -> EngineResult<Produced>
// Produced { staged: Staged::File | Staged::Dir, expectation: Expectation, notes: Vec<TaskNote> }
```

- Create outputs only with `ctx.output.temp_file(&task.output().dir)` or `temp_dir(...)`. Never write to the final path and never publish.
- Call `ctx.cancel.check()?` between steps (decode / resize / encode, each page, each entry); wrap long reads in `CancellableReader` or a progress reader.
- Report progress with `ctx.progress(0.0..=1.0)`; jumps of 10% or more are always delivered.
- Return precise `ErrorCode`s (`EngineError::read` for inputs, `EngineError::write` for outputs).
- Return an `Expectation` the validator can check (format, size, pages, entries, streams, file count).

## Adding a format or operation (all steps required)

1. `format.rs`: variant, extensions, family, signature in `sniff`, plus a unit test.
2. `capability.rs`: inputs and outputs, presets, input mode, availability rule (and encoder check for FFmpeg).
3. `operation.rs` options struct (`#[derive(TS)]`, camelCase) and `plan.rs` validation and output naming.
4. Executor and validator support.
5. Integration tests in `crates/omnidrop-engine/tests/` with generated fixtures (see the `conversion-testing` skill).
6. UI: `src/lib/formats.ts`, i18n hints, `docs/FORMATS.md`, `CHANGELOG.md`.
7. `cargo test --workspace` (regenerates bindings) and commit `src/lib/bindings`.

## Safety rules

- External programs: `tools::tool_command(path).args(vec)`; inputs and outputs via `ffmpeg_file_arg`; enforce `limits.media_timeout`; kill on cancel.
- Untrusted-input limits live in `limits.rs`. Image decoding uses `limits.image_limits()`; PDFs use `LoadOptions::max_decompressed_size`.
- Archive paths go through `names::safe_relative_path`; skip links and devices; count bytes as they are written (headers lie); drain skipped 7z entries.
- Originals are protected by `OutputManager`; never rename or remove inputs.

## Verifiable criteria

- [ ] `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check` are clean.
- [ ] Success, corrupt input, cancellation and collision tests exist for the new path.
- [ ] No temporary `.omnidrop-*` files remain after any test (`no_temp_files`).
