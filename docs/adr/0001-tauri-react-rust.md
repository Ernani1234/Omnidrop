# 0001 — Tauri 2 with a React interface and a Rust engine

## Context

OmniDrop needs a polished desktop UI on Windows, macOS and Linux, native control over files and
processes, and a small download. The brief proposed Tauri 2 + React/TypeScript + Rust and named
Electron as a valid alternative to be decided by a prototype.

## Decision

Use Tauri 2. The interface is React 19 + TypeScript + Tailwind CSS 4 with Radix primitives; all file
inspection, planning, conversion and output handling is Rust.

## Consequences

- Installers are small (the system web view is reused) and the engine runs natively without a
  Node.js runtime.
- The UI renders in three different web engines (WebView2, WebKit on macOS, WebKitGTK); the build
  targets ES2022/Safari 15 and avoids engine-specific features.
- The engine is a separate crate with no UI dependency, which keeps it testable and reusable
  (a CLI could be added later).
