
---

### Which file should I download?

| System | File |
| --- | --- |
| Windows 10/11 (64-bit) | `OmniDrop_<version>_x64-setup.exe` (recommended) or the `.msi` |
| macOS 11 or newer (Apple Silicon and Intel) | `OmniDrop_<version>_universal.dmg` |
| Ubuntu, Debian, Linux Mint | `OmniDrop_<version>_amd64.deb` |
| Fedora, openSUSE | `OmniDrop-<version>-1.x86_64.rpm` |
| Other Linux distributions | `OmniDrop_<version>_amd64.AppImage` |

### First launch

These builds are not signed with a paid certificate yet, so the system asks for confirmation once:

- **Windows:** if SmartScreen shows "Windows protected your PC", choose **More info → Run anyway**.
- **macOS:** right-click OmniDrop in Applications and choose **Open**, then confirm. If macOS says the app is damaged, run `xattr -dr com.apple.quarantine /Applications/OmniDrop.app` in Terminal.
- **Linux (AppImage):** make the file executable (`chmod +x OmniDrop_*.AppImage`) and run it.

### Audio and video

Audio and video conversion uses the FFmpeg installed on your computer (OmniDrop never downloads it):
`winget install Gyan.FFmpeg` (Windows), `brew install ffmpeg` (macOS) or `sudo apt install ffmpeg` (Debian/Ubuntu).
Everything else works without it.

OmniDrop processes files only on your computer; the app itself makes no network requests. See the [privacy policy](https://github.com/Ernani1234/Omnidrop/blob/main/docs/PRIVACY.md).
