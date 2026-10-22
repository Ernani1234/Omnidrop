# 0006 — Generate TypeScript contracts from Rust

## Context

Requests, plans, events and errors cross the IPC boundary. Hand-written TypeScript copies drift.

## Decision

Every IPC type derives `ts_rs::TS`. `cargo test` writes `src/lib/bindings/*.ts`; the files are
committed, and CI fails if regenerating them changes anything.

## Consequences

- Renaming a field or adding an error code breaks the TypeScript build immediately.
- The UI never re-implements engine rules: it asks the planner for a preview.
