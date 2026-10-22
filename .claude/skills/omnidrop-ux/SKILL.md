---
name: omnidrop-ux
description: UX rules for OmniDrop screens — simple flows, empty states, microcopy, actionable errors, visual consistency and fewer steps. Use when adding or changing anything the user sees.
---

# OmniDrop UX

## Rules

1. **One primary action per context.** Empty: "Choose files". Configuring: the CTA button. Running: progress + Cancel. Done: "Open folder". Only one `variant="primary"` button may be visible per region.
2. **Simple first.** Lossy formats show the three presets (Smallest file / Balanced / Best quality). Everything else goes inside `<Disclosure title={t("advanced.title")}>`.
3. **Only offer what works here.** Operations and formats come from `tools.capabilities`; never hard-code availability. Unavailable items stay visible, disabled, with the reason and a way to fix it.
4. **Never estimate before converting.** Show measured results after the job (space saved, per-file size change).
5. **Errors are actionable.** Use `ErrorMessage` (title + hint + folded technical details). Add `error.<code>` and `error.<code>.hint` in both languages for any new `ErrorCode`.
6. **Counts in labels.** Buttons say what happens: `t(plural("cta.files", n), { verb, count })`.
7. **Plain words.** No "codec", "bitrate", "CRF" outside advanced options and technical details.
8. **Keep the queue honest.** Rows show the real format, mismatched extensions, why a file is skipped, and per-file status.

## Microcopy (must stay verbatim)

| Situation | pt-BR key |
| --- | --- |
| Empty | `drop.title` — "Solte seus arquivos aqui para começar." |
| Choosing | `op.question` — "O que você gostaria de fazer?" |
| Running | `running.imageConvert` — "Convertendo {current} de {total} arquivos…" |
| Success | `done.success` — "Tudo pronto. Seus arquivos estão na pasta de saída." |
| Unreadable | `error.unreadable` |
| Unsupported | `error.unsupportedConversion` |

## Procedure for a UI change

1. Add strings to `src/i18n/pt-BR.ts` first, then `src/i18n/en.ts` (the type checker enforces parity).
2. Use tokens (`bg-surface`, `text-muted`…) and primitives from `src/components/ui/primitives.tsx`; no raw colours.
3. Run `npm run dev` and open `http://localhost:1420/?demo=files` (also `demo=running`, `demo=done`, `theme=dark`, `lang=en`). Screenshot with headless Edge/Chrome at 1280×800 and 780×620.
4. Check the empty, files, running, done and error states in both themes.
5. For release screenshots use the real app (`npm run tauri build -- --no-bundle`), never the simulated backend.

## Verifiable criteria

- [ ] `npm test` passes (includes the microcopy test).
- [ ] No new primary button competes with an existing one.
- [ ] Every new control has a visible label or `aria-label`.
- [ ] Layout works at the minimum window size (760×560) without horizontal scrolling.
