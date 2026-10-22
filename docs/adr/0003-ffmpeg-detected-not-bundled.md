# 0003 — Use the installed FFmpeg instead of bundling it

## Context

Audio/video needs FFmpeg. Bundling it adds 80–120 MB per platform, and the useful encoders (x264,
LAME) bring GPL obligations. Downloading it on demand would make the app use the network.

## Decision

Detect FFmpeg/ffprobe on the computer (custom path chosen in a native dialog, `PATH`, and
package-manager folders that macOS GUI apps do not inherit). Read the encoder list and offer only
the outputs that FFmpeg can produce. Without FFmpeg, audio/video operations are listed as unavailable
with installation instructions.

## Consequences

- Small installers, no codec licensing in our distribution, and no network use.
- Results depend on the user's FFmpeg build; the capability registry reflects that per machine.
- Processes are started with argument vectors and the `file:` protocol, killed on cancellation or
  timeout, and never show a console window on Windows.
