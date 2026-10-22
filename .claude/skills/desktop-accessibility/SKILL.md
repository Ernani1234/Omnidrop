---
name: desktop-accessibility
description: Accessibility checklist for OmniDrop — keyboard, focus, screen readers, contrast, scaling and reduced motion. Use for any UI change and before releases.
---

# Desktop accessibility

## Requirements

- **Keyboard:** every action reachable with Tab and Shift+Tab; radio groups use arrow keys (Radix); dialogs trap focus and close with Esc. Shortcuts: Ctrl/⌘+O, Ctrl/⌘+Shift+O, Ctrl/⌘+Enter, Ctrl/⌘+A, Delete, Ctrl/⌘+comma.
- **Focus:** the global `:focus-visible` outline (2 px accent) must never be removed.
- **Names and roles:** icon-only buttons use `IconButton` (aria-label plus a tooltip that appears on focus). Inputs have a `<label>` or `Field`. Decorative icons get `aria-hidden`.
- **Announcements:** call `announce()` from the store for added and removed files; job start, 25% milestones and results are announced by `useJobAnnouncements` through `LiveRegion` (polite). Status text uses `role="status"`; `role="alert"` only for errors shown in response to an action.
- **Contrast:** WCAG AA (4.5:1 for text, 3:1 for UI parts) in both themes.
- **Motion:** animate only with `motion-safe:` or rely on the global `prefers-reduced-motion` rule in `src/styles.css`.
- **Never colour alone:** status always pairs an icon and text.

## Procedures

**Contrast.** For every changed token pair compute `(L1 + 0.05) / (L2 + 0.05)` with relative luminance
from sRGB; soft backgrounds in dark mode are the colour composited over `--surface` at its alpha.

**Running app (Windows UI Automation).** Start the release build, then list every control and
confirm each has a name and the right role:

```powershell
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$p = Get-Process omnidrop | Where-Object { $_.MainWindowHandle -ne 0 }
$root = [System.Windows.Automation.AutomationElement]::FromHandle($p.MainWindowHandle)
$all = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
foreach ($e in $all) { "{0} | {1}" -f $e.Current.ControlType.ProgrammaticName, $e.Current.Name }
```

**Screen readers** to try: NVDA (Windows), VoiceOver (macOS, ⌘F5), Orca (Linux).

## Verifiable criteria

- [ ] UI tests cover keyboard paths for new controls (`@testing-library/user-event`).
- [ ] No unnamed buttons, radios, checkboxes or combo boxes in the UI Automation listing.
- [ ] All token pairs are at least 4.5:1 in light and dark.
- [ ] Usable at 200% system text scaling and at the minimum window size of 760×560.
