# Contributing to OmniDrop

Thanks for helping! OmniDrop aims to be simple for users and boringly reliable underneath, so
changes are judged by those two things first.

## Before you start

- For bugs, open an issue with steps to reproduce, your OS and OmniDrop version (Settings → About).
  Attach logs only after checking they contain nothing private.
- For new formats or operations, open an issue first describing the use case and the engine you
  would use (license, size, platform support).

## Development

See [docs/BUILDING.md](docs/BUILDING.md). In short:

```bash
npm ci
npm run tauri dev
cargo test --workspace && npm test && npm run typecheck
```

## Ground rules

- **Never claim support you have not implemented and tested.** A format appears in the UI only when
  its executor, validation and tests exist (see "Adding a new format" in
  [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)).
- **Keep originals safe.** Executors return staged results; only the OutputManager publishes.
- **No shell strings.** Start processes with argument vectors and validated parameters.
- **Paths never come from the UI.** Use ids; open native dialogs from Rust.
- **Plain language.** Every user-visible string lives in `src/i18n/pt-BR.ts` and `src/i18n/en.ts`;
  errors explain what happened and what to do.
- **Accessibility is part of done:** keyboard, focus, labels, contrast, reduced motion.
- **No network access** without an explicit, documented, opt-in design discussed in an issue.

## Definition of done

A change is complete when it works end to end in the running app, has automated tests for success
and failure cases, handles errors with actionable messages, follows the
[design system](docs/DESIGN_SYSTEM.md), and keeps `cargo clippy -- -D warnings`, `cargo fmt`,
`npm run typecheck` and all tests green. Update `CHANGELOG.md` and the docs when behaviour changes.

## Commits and pull requests

- Small, focused pull requests with a clear description and screenshots for UI changes.
- Regenerated `src/lib/bindings` must be committed together with Rust type changes.
- By contributing you agree that your work is licensed under the MIT License.
