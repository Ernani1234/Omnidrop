# Architecture

OmniDrop is a Tauri 2 application. A React interface talks to a thin Rust shell, which drives a
conversion engine that knows nothing about the UI.

```
┌──────────────────────────── src/ (React + TypeScript) ────────────────────────────┐
│ EmptyState · FileQueue · OperationPanel · RunBar · Dialogs        zustand store    │
│                       lib/api.ts  ── invoke / events ──                             │
└──────────────────────────────────────┬─────────────────────────────────────────────┘
                                       │ ids + typed requests (ts-rs generated types)
┌──────────────────────────── src-tauri/ (shell) ───────────────────────────────────┐
│ commands.rs: bootstrap, pick_*, preview_plan, start_job, cancel_*, open/reveal…    │
│ lib.rs: window, native drag-and-drop, launch arguments, plugins (dialog, opener,   │
│         log)                         state.rs: queue, jobs, authorized results     │
└──────────────────────────────────────┬─────────────────────────────────────────────┘
                     ┌─────────────────┴──────────────────┐
┌──── crates/omnidrop-app ────┐       ┌──────────── crates/omnidrop-engine ────────────┐
│ settings · history ·        │       │ inspector → capability → plan → job → executors │
│ payloads · thumbnails       │       │ → validator → output        (+ tools, limits)   │
└─────────────────────────────┘       └─────────────────────────────────────────────────┘
```

## Engine pipeline

| Module | Responsibility |
| --- | --- |
| `format` | `FileFormat`, families, extensions, and signature sniffing (PNG, JPEG, WebP, AVIF/ISO-BMFF brands, TIFF, PDF, MP3/ADTS, FLAC, Ogg, EBML/WebM, RIFF WAV/AVI, ZIP, 7z, TAR, OLE). |
| `inspector` (FileInspector) | Expands folders safely, detects the real format, reads metadata (dimensions, animation, pages, duration via ffprobe, archive entries) and records problems as structured errors instead of failing. |
| `capability` (CapabilityRegistry) | Declares every operation: inputs, outputs available *on this machine*, input mode (per file or combined), presets, cancellation behaviour, external dependency, size limit and availability with a reason. |
| `plan` (OperationPlanner) | Validates options, splits files into accepted and skipped (with a reason each), resolves output folders and names, adds notes (transparency flattening, first frame only, audio from video) and counts expected outputs when exact. |
| `job` (TaskQueue) | Runs tasks on scoped worker threads with concurrency bounded per operation, emits `JobEvent`s with rate-limited progress, supports job-wide and per-task cancellation, and isolates panics and failures. |
| `executors` (ConversionExecutor) | `image`, `pdf`, `archive`, `media`. Each produces a *staged* result plus an `Expectation`; none publishes anything itself. |
| `validator` (OutputValidator) | Re-reads the staged result: format signature, pixel size, PDF page count, archive entry count, media streams via ffprobe, file counts in folders. |
| `output` (OutputManager) | Creates temporary files/folders inside the destination (same filesystem, so publishing is a rename), applies the collision rule with no-clobber renames, never replaces job inputs or earlier results of the same job, cleans up on drop. |
| `tools` | Finds FFmpeg/ffprobe (custom path, PATH, package-manager folders), reads versions and encoders, runs processes with argument vectors, timeouts and no console window. |
| `limits` | Pixel/memory limits for images, PDF size, render size, archive entries/bytes/ratio, import size and depth, media time budget. |
| `names`, `pages`, `cancel` | File-name sanitization (Windows-reserved names, invalid characters), safe archive paths, page-range parsing, cancellation tokens and cancellable readers. |

### Contracts

All types crossing the IPC boundary derive `serde` and `ts_rs::TS`. Running `cargo test` regenerates
`src/lib/bindings/*.ts`; CI fails if the committed files are out of date, so the UI and the engine
cannot drift apart. Errors are `EngineError { code, detail }`: the UI maps `code` to plain-language
copy and a suggested action, and shows `detail` only under "Technical details".

### Concurrency

| Operation | Parallel tasks |
| --- | --- |
| Image conversion | up to 4 (2 for AVIF, whose encoder is itself multi-threaded) |
| PDF to images | up to 2 |
| Other PDF operations | 2 |
| Audio | up to 2 |
| Video | 1 (FFmpeg uses every core) |
| Archives | 1 |

Cancellation is cooperative between steps (decode → resize → encode, page by page, entry by entry)
and immediate for FFmpeg, whose process is killed. Queued tasks stop at once.

## Trust boundary

The webview is treated as untrusted input:

- Paths never come from the UI. Files enter through native drag-and-drop events handled in Rust,
  native dialogs opened by Rust, or launch arguments. The UI refers to files, results, jobs and
  history entries by id.
- The output folder and the FFmpeg executable can only be set through native dialogs; the settings
  patch the UI may send has no path fields.
- Results can be opened or revealed only if this session created them.
- External links are a closed enum (`ExternalLink`), not URLs.
- The capability file grants the window `core:default` and `core:window:allow-set-theme` only; the
  dialog and opener plugins are used from Rust, not exposed to JavaScript.
- A strict CSP allows only bundled scripts, styles and fonts, and `data:` images for thumbnails.

## Desktop shell

`src-tauri` keeps no logic that needs testing: it wires commands to the engine, runs jobs on
background threads, records results the UI may open, writes history, and forwards events
(`omnidrop://files`, `inspecting`, `drag`, `job`, `tools`). Testable shell logic lives in
`crates/omnidrop-app` because test binaries that link the webview runtime cannot start on Windows
without an application manifest.

## Frontend

- **State:** a single zustand store (`src/store/app.ts`) holds the queue, operation options per
  operation, the plan preview, the running job and dialogs.
- **Plan preview:** every change of files, operation, options or output settings asks the backend
  planner for a preview (debounced). The UI never re-implements compatibility rules; the button
  label, skipped files and notes come from the real plan.
- **Design system:** CSS custom properties for colour tokens (light and dark), Tailwind CSS 4
  utilities mapped to them, Radix primitives for radio groups, switches, checkboxes, dialogs,
  progress, tooltips and disclosure. See [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md).
- **i18n:** `src/i18n/pt-BR.ts` defines the keys; `en.ts` must provide every key (checked by the
  type system and by tests, including placeholders).
- **Browser preview:** in `vite dev` outside Tauri, `src/lib/mock.ts` simulates the backend for UI
  work. It is never included in production builds.

## Adding a new format or operation

1. Add the format to `FileFormat` (extensions, family, signature in `sniff`).
2. Declare it in `CapabilityRegistry` (inputs/outputs, presets, availability rules).
3. Teach the planner the output name and any option validation.
4. Implement the executor so it returns a staged result and an `Expectation`.
5. Make the validator able to check that expectation.
6. Add UI labels/hints (`FORMATS`, i18n) — the operation list and forms pick it up.
7. Add integration tests with generated fixtures, then update [FORMATS.md](FORMATS.md).
