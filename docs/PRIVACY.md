# Privacy policy

*Last updated: 22 October 2026 · applies to OmniDrop 0.1.0 and later*

OmniDrop is built to work entirely on your computer. This policy describes exactly what the app
does with your data.

## What OmniDrop does

- **Your files stay on your computer.** Files are read and written only on your disks. OmniDrop does
  not upload, sync or share them.
- **No network requests.** The app itself does not connect to the internet: no telemetry, analytics,
  crash reporting, update checks or remote fonts.
- **Links open only when you click them.** Buttons such as "ffmpeg.org", "Source code" and "Report
  a problem" open a fixed web address in your default browser. Nothing is sent by OmniDrop.

## What is stored on your computer

| Data | Where | Why | How to remove |
| --- | --- | --- | --- |
| Settings (theme, language, output folder, name-collision rule, FFmpeg location) | App config folder, `settings.json` | Remember your preferences | Delete the file |
| Recent jobs (operation, date, counts, output folder, original file paths) | App data folder, `history.json` | "Recent" list and "Add files again" | **Recent → Clear history** |
| Diagnostic logs (start-up, FFmpeg version, error codes and technical messages, which may include file names) | App log folder | Help you or a maintainer understand a problem | Delete the folder (**Settings → Open logs**) |
| Web view cache | App local data folder | Required by the system web view | Delete the folder |

Folder locations: `%APPDATA%\io.github.ernani1234.omnidrop` and
`%LOCALAPPDATA%\io.github.ernani1234.omnidrop` on Windows,
`~/Library/Application Support/io.github.ernani1234.omnidrop` and `~/Library/Logs/…` on macOS,
`~/.config/io.github.ernani1234.omnidrop`, `~/.local/share/…` on Linux.

Logs are never sent anywhere. If you share them in a bug report, review them first.

## Third-party components

- **FFmpeg** is not bundled. When you use audio or video features, OmniDrop runs the FFmpeg already
  installed on your computer, locally, with your files as input. FFmpeg does not access the network
  in these conversions.
- **System web view.** The interface is displayed by your operating system's web view (Microsoft Edge
  WebView2 on Windows, WebKit on macOS and Linux). These are system components maintained by their
  vendors and may follow their own update policies. OmniDrop loads only its bundled interface and
  restricts it with a strict content security policy.
- **Windows installer.** If WebView2 is missing (rare on Windows 10/11), the installer downloads it from
  Microsoft.

## Changes

If a future version adds any feature that uses the network, it will be optional, described here
before release, and clearly indicated in the app.

Questions: open an issue at <https://github.com/Ernani1234/Omnidrop/issues>.
