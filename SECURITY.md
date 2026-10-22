# Security policy

## Supported versions

Security fixes are released for the latest version only.

## Reporting a vulnerability

Please **do not open a public issue**. Use GitHub's
[private vulnerability reporting](https://github.com/Ernani1234/Omnidrop/security/advisories/new)
and include the OmniDrop version, your OS, and the steps or a sample file that reproduce the problem.
You will get an acknowledgement within a week.

Especially relevant reports: crafted files that write outside the chosen folder, crash or hang the
app, exhaust memory or disk despite the limits, replace an original file, or let the interface reach
paths or programs it should not.

## Design measures

OmniDrop processes untrusted files. The engine applies size, pixel, page, entry-count, expansion-ratio
and time limits; extracts archives only into new folders after validating every path; never follows
links; starts external programs without a shell; and publishes results only after validation.
The web view cannot name file-system paths. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#trust-boundary).
