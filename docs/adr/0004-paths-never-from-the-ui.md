# 0004 — File paths never come from the web view

## Context

The interface runs in a web view. If it could name arbitrary paths, any script injection would
turn into arbitrary file reads and writes.

## Decision

Paths enter the backend only through native drag-and-drop events (handled in Rust), native dialogs
opened by Rust, or launch arguments. The UI refers to files, results, jobs and history entries by
opaque ids. The output folder and FFmpeg location can only be set through native dialogs. Results
can only be opened or revealed if this session created them. The window has only core Tauri
permissions plus setting its own theme, and a strict CSP.

## Consequences

- A compromised page cannot read or write outside what the user selected.
- Features such as "type a path" are intentionally absent.
