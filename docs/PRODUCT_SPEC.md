# Product specification

OmniDrop is a cross-platform desktop app that converts, transforms and organizes files with the
least possible effort. A simple interface sits on top of a modular, safe and testable conversion
engine.

This document turns the original technical and UX brief into requirements with verifiable
acceptance criteria. Each criterion names the automated test that covers it where one exists.

## Principles

1. **Simple** — add files, choose an operation, start.
2. **Private by default** — local processing; no uploads, no telemetry.
3. **Reliable** — validate results, preserve originals.
4. **Cross-platform** — Windows, macOS and Linux.
5. **Modular** — new formats plug into the engine without rewriting the UI.
6. **Transparent** — supported formats, limitations and dependencies are stated plainly. An
   operation is only offered when its engine is implemented, validated and available on the machine.

## Main flow

| Step | Responsibility | What the user sees |
| --- | --- | --- |
| 1. Input | Drop, or pick files and folders. | Files appear in the queue. |
| 2. Inspection | Detect real format, size, dimensions/pages/duration. | What was recognized, including extension mismatches. |
| 3. Operation | Show only operations compatible with the files. | One simple choice. |
| 4. Configuration | Safe presets first; advanced parameters folded. | Settings without jargon. |
| 5. Execution | Queue with progress, bounded concurrency and cancellation. | Per-file and overall status. |
| 6. Validation | Check every result before publishing it. | Trustworthy results that are easy to find. |

## Requirements and acceptance criteria

### R1 — Input

- R1.1 Dropping files or folders anywhere on the window adds them; folders are read recursively, skipping hidden and system files and never following links. *(engine `inspector::tests::expands_folders_without_hidden_files`)*
- R1.2 Files can also be added with native dialogs (files or folder), with <kbd>Ctrl/⌘+O</kbd>, and via "Open with". *(UI `empty state` test)*
- R1.3 Importing stops at 10,000 files and says so. *(engine `import_limit_truncates`)*
- R1.4 The same file is not added twice.

### R2 — Inspection

- R2.1 The real format is detected from content, not the extension. *(engine `format::tests`, `detects_misleading_extension_and_unknown_content`)*
- R2.2 A mismatch between extension and content is shown as a warning, and the file is still processed by its real format. *(engine `misleading_extension_is_converted_by_content`)*
- R2.3 Unknown, unreadable or password-protected files are explained in plain language and excluded from conversions. *(engine `unknown_and_unreadable_inputs_are_explained_in_the_plan`, `password_protected_pdf_is_explained`; UI `explains unreadable files`)*
- R2.4 Image dimensions, PDF page counts, media duration/resolution (with FFmpeg) and archive entry counts are shown.

### R3 — Choosing an operation

- R3.1 Only operations that accept at least one file are listed, the dominant file type first; "Compress" is always available last. *(UI `operation choices` tests)*
- R3.2 Operations that exist but cannot run here (FFmpeg missing) are shown disabled with the reason and a way to fix it. *(engine `media_is_unavailable_without_ffmpeg`; UI `explains that audio and video need FFmpeg`)*
- R3.3 Files not used by the selected operation are visibly marked; the plan states how many will be skipped and why.

### R4 — Configuration

- R4.1 Lossy formats offer "Smallest file / Balanced / Best quality"; lossless formats explain there is nothing to adjust.
- R4.2 Advanced options are folded: manual quality, lossless WebP, resize (fit/percentage, never enlarging when fitting), background colour for JPEG, video resolution cap and audio removal, page selections, archive names.
- R4.3 Size estimates are not shown before converting, because no reliable method exists for every format. After a job, the actual space saved is reported.
- R4.4 Output location (next to originals or a chosen folder) and the collision rule are always visible; "Original files are never changed" is stated.

### R5 — Execution

- R5.1 Work runs outside the UI thread with concurrency bounded per operation type. *(engine `job::concurrency`)*
- R5.2 Progress is reported per file and in total through events, never polling. *(engine `every_task_reports_intermediate_progress`)*
- R5.3 The job can be cancelled before or during execution; single items can be cancelled. Cancelling leaves no partial files. *(engine `cancelling_before_start_processes_nothing`, `cancelling_during_a_batch_stops_remaining_items_cleanly`, `cancelling_one_task_keeps_the_others`, `cancelling_kills_ffmpeg_and_leaves_nothing`)*
- R5.4 A failing item never stops the others. *(engine `corrupt_file_fails_alone_and_batch_continues`)*
- R5.5 Batches of hundreds of files work. *(engine `large_batch_completes` — 200 files)*

### R6 — Validation and output

- R6.1 Results are written to a temporary file in the destination folder and published only after validation (format, dimensions, page count, archive entries, media streams). *(engine `validator::tests`, `output::tests`)*
- R6.2 Originals are never replaced, even with the "Replace" rule. *(engine `overwrite_replaces_other_files_but_never_inputs`, `rotate_selected_pages`)*
- R6.3 Collisions follow the chosen rule: keep both (`name (1).ext`), replace, or skip. Two results of the same job never overwrite each other. *(engine `collisions_follow_policy_and_never_touch_existing_files`, `same_name_inputs_in_one_batch_do_not_overwrite_each_other`)*
- R6.4 Errors writing the output (permissions, full disk) are reported with an action. *(engine `read_only_destination_reports_actionable_error`, `maps_io_errors_by_direction`)*
- R6.5 After a job: "Open folder", "View results" and "Repeat" (or "Retry the failed ones"). *(UI `runs a job and reports progress and results`)*

### R7 — Security

- R7.1 External programs are started with argument vectors, never through a shell; inputs use FFmpeg's `file:` protocol. *(engine `file_arg_has_protocol_prefix`)*
- R7.2 Archive extraction rejects absolute paths, `..` components, drive prefixes and links; size, entry-count and expansion-ratio limits stop bombs and leave nothing behind. *(engine `zip_slip_entries_are_skipped`, `decompression_bomb_is_stopped_and_cleaned_up`, `entry_count_limit_applies`, `tar_links_are_not_extracted`)*
- R7.3 Image decoding has pixel and memory limits; PDF loading limits stream decompression; media conversions have time limits.
- R7.4 The webview cannot name arbitrary paths: files, output folders and the FFmpeg location only come from native drag-and-drop or dialogs.
- R7.5 A strict content security policy is set; the webview has only core permissions.

### R8 — Accessibility

- R8.1 Every action is reachable by keyboard; radio groups use arrow keys. *(UI `lets the user pick a format with the keyboard`, `removes files with the keyboard`)*
- R8.2 Focus is always visible; colours meet WCAG AA contrast in both themes.
- R8.3 Controls expose names and roles to assistive technology (verified with Windows UI Automation on the running app); progress and results are announced through a live region.
- R8.4 `prefers-reduced-motion` disables animations.

### R9 — Privacy

- R9.1 No network requests are made by the app. Links (FFmpeg download, source code) open in the user's browser only when clicked.
- R9.2 Logs contain technical error details only and stay on the computer.

## Microcopy

| Situation | Text |
| --- | --- |
| Empty | "Solte seus arquivos aqui para começar." / "Drop your files here to get started." |
| Choosing | "O que você gostaria de fazer?" / "What would you like to do?" |
| Running | "Convertendo 8 de 30 arquivos…" / "Converting 8 of 30 files…" |
| Success | "Tudo pronto. Seus arquivos estão na pasta de saída." / "All done. Your files are in the output folder." |
| Unreadable | "Não foi possível ler este arquivo. Ele pode estar corrompido ou usar um formato não suportado." |
| Unsupported | "Ainda não oferecemos esta conversão. Veja as opções disponíveis para este arquivo." |

The UI test suite checks that these strings are used verbatim (`keeps the microcopy defined in the specification`).

## Scope by phase

| Area | Status in 0.1.0 |
| --- | --- |
| Images (PNG, JPEG, WebP, AVIF, TIFF; batch, resize, transparency) | Done |
| PDF (merge, split, rotate, export pages as images) | Done |
| Audio/video (MP3, WAV, FLAC, AAC, MP4, MKV, WebM, depending on local FFmpeg codecs) | Done |
| Archives (ZIP, TAR, TAR.GZ, 7z; create and extract with path validation) | Done |
| Editable documents | Later — recognized and explained, not converted |
