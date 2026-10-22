# Design system

OmniDrop should feel like a calm, modern desktop tool — warm rather than clinical, and never like a
generic admin dashboard. One primary action per context, plain language, and nothing that moves
without a reason.

## Tokens

All colours are CSS custom properties in [`src/styles.css`](../src/styles.css), redefined for the
dark theme under `[data-theme="dark"]` and exposed to Tailwind through `@theme inline`
(`bg-surface`, `text-muted`, `border-border-strong`…). Components never use raw hex values.

| Token | Light | Dark | Use |
| --- | --- | --- | --- |
| `--bg` | `#f6f4f0` | `#121214` | Window background (warm paper / near black) |
| `--surface` | `#ffffff` | `#1b1b1f` | Cards, dialogs |
| `--surface-2` / `-3` | `#f0ede8` / `#e8e4dd` | `#232329` / `#2c2c33` | Insets, segmented controls, tracks |
| `--border` / `-strong` | `#e2ddd5` / `#cbc4b8` | `#2f2f37` / `#45454f` | Dividers, inputs |
| `--text` | `#1c1a17` | `#f2f1ee` | Primary text |
| `--text-muted` | `#615b52` | `#b3afa8` | Secondary text (≥ 5:1 on backgrounds) |
| `--accent` | `#5b4cf0` | `#8f84ff` | Primary actions, selection, focus |
| `--success` / `--warning` / `--danger` | `#17794f` / `#8a5300` / `#b42e2e` | `#5fd3a0` / `#f2b552` / `#ff8c82` | Status, always paired with an icon and text |
| `--fam-*` | per family | per family | Tints for image, PDF, audio, video, archive, document |

Every text/background pair meets WCAG AA (4.5:1 for body text). Status is never conveyed by colour
alone.

## Typography

- **Bricolage Grotesque** (variable) for the brand, headings and big numbers — a distinctive grotesque
  that gives the app its personality.
- **Manrope** (variable) for everything else — open shapes that stay legible at 12–15 px.
- Both are bundled (OFL-1.1); nothing is fetched from the network. Numbers that change use tabular
  figures.

## Layout

```
┌ Header: brand · "Local processing" · Recent · Settings ───────────────────────┐
│ Empty: large drop zone (primary: Choose files) + quick actions                 │
│ With files: [ File queue ............ ] [ What would you like to do?        ] │
│             thumbnails, format, size,   operations · format · preset ·       │
│             status per file             advanced (folded) · Save to · CTA     │
├ Run bar: progress + Cancel  →  summary + Open folder / View results / Repeat ─┤
```

The two-column layout collapses to one column below 768 px; the window's minimum size is
760 × 560.

## Components

| Component | Notes |
| --- | --- |
| Button | `primary` (one per context), `secondary`, `ghost`, `danger`; sizes sm/md/lg; icons are decorative (`aria-hidden`). |
| IconButton | Always has an accessible label and a tooltip that also appears on keyboard focus. |
| ChoiceChips | Radix radio group shown as chips (formats). Arrow keys move the choice. |
| Segmented | Radix radio group for presets and short exclusive choices. |
| Switch, Checkbox, Disclosure, Dialog, ProgressBar, Tooltip | Radix primitives with the tokens above. |
| NativeSelect | Native `<select>` for long lists — best keyboard and screen-reader behaviour on every OS. |

## Motion

Transitions are 150–300 ms and only confirm state (hover, selection, progress). The drop zone
pulses gently while files hover over it. `prefers-reduced-motion: reduce` turns all animation off.

## Writing

- Speak to the user, in plain words: "Choose a format", not "Select output codec".
- Errors say what happened and what to do next; technical details are folded under "Technical
  details" with a copy button.
- Buttons say what will happen, with the count: "Convert 30 files", "Merge 3 files".
- Never promise what is not implemented. Don't estimate sizes before converting; report the real
  savings afterwards.

The canonical copy lives in [`src/i18n/pt-BR.ts`](../src/i18n/pt-BR.ts) and
[`src/i18n/en.ts`](../src/i18n/en.ts).

## Accessibility checklist

- Every control is reachable and operable by keyboard; focus is always visible (2 px accent outline).
- Controls expose names and roles (verified with Windows UI Automation on the running app).
- Progress milestones, added files and results are announced through a polite live region.
- Contrast meets WCAG AA in both themes; text scales with the system font size.
- Animations respect reduced motion.
