//! Discovery and safe invocation of optional external tools (FFmpeg).
//!
//! Processes are always started with an argument vector, never through a
//! shell, and with a time limit.

use serde::Serialize;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolInfo {
    pub path: String,
    pub version: String,
    #[serde(skip)]
    pub executable: PathBuf,
    #[serde(skip)]
    pub encoders: BTreeSet<String>,
}

impl ToolInfo {
    pub fn has_encoder(&self, name: &str) -> bool {
        self.encoders.contains(name)
    }
}

/// External tools found on this machine.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolSet {
    pub ffmpeg: Option<ToolInfo>,
    pub ffprobe: Option<ToolInfo>,
}

impl ToolSet {
    /// No external tools; useful for tests and as a starting state.
    pub fn none() -> Self {
        Self::default()
    }

    /// Looks for FFmpeg, preferring `custom` (a file or its folder) when given.
    pub fn discover(custom: Option<&Path>) -> Self {
        let ffmpeg = find_tool("ffmpeg", custom).and_then(|path| probe_ffmpeg(&path));
        let ffprobe_dir = ffmpeg
            .as_ref()
            .and_then(|f| f.executable.parent().map(Path::to_path_buf));
        let ffprobe = find_tool("ffprobe", ffprobe_dir.as_deref().or(custom))
            .and_then(|path| probe_version(&path));
        Self { ffmpeg, ffprobe }
    }
}

fn exe_name(base: &str) -> String {
    if cfg!(windows) {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

fn find_tool(base: &str, preferred: Option<&Path>) -> Option<PathBuf> {
    let exe = exe_name(base);
    if let Some(preferred) = preferred {
        let candidate = if preferred.is_dir() {
            preferred.join(&exe)
        } else {
            preferred.with_file_name(&exe)
        };
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if let Ok(found) = which::which(base) {
        return Some(found);
    }
    // GUI apps on macOS do not inherit the shell PATH, and package managers
    // on every OS install into well-known folders.
    let mut dirs: Vec<PathBuf> = vec![];
    if cfg!(target_os = "macos") {
        dirs.extend(["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"].map(PathBuf::from));
    } else if cfg!(target_os = "linux") {
        dirs.extend(["/usr/bin", "/usr/local/bin", "/snap/bin"].map(PathBuf::from));
    } else if cfg!(windows) {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let local = PathBuf::from(local);
            dirs.push(local.join("Microsoft").join("WinGet").join("Links"));
        }
        if let Some(profile) = std::env::var_os("USERPROFILE") {
            dirs.push(PathBuf::from(profile).join("scoop").join("shims"));
        }
        dirs.push(PathBuf::from(r"C:\ProgramData\chocolatey\bin"));
        dirs.push(PathBuf::from(r"C:\ffmpeg\bin"));
    }
    dirs.into_iter()
        .map(|d| d.join(&exe))
        .find(|candidate| candidate.is_file())
}

fn probe_version(path: &Path) -> Option<ToolInfo> {
    let output = run_capture(path, ["-hide_banner", "-version"], Duration::from_secs(10)).ok()?;
    if !output.success {
        return None;
    }
    let first = output.stdout.lines().next()?.trim().to_string();
    let version = first
        .split_whitespace()
        .nth(2)
        .unwrap_or(first.as_str())
        .to_string();
    Some(ToolInfo {
        path: path.to_string_lossy().into_owned(),
        version,
        executable: path.to_path_buf(),
        encoders: BTreeSet::new(),
    })
}

fn probe_ffmpeg(path: &Path) -> Option<ToolInfo> {
    let mut info = probe_version(path)?;
    let output = run_capture(path, ["-hide_banner", "-encoders"], Duration::from_secs(10)).ok()?;
    info.encoders = parse_encoders(&output.stdout);
    Some(info)
}

/// Parses `ffmpeg -encoders` output into encoder names.
pub fn parse_encoders(listing: &str) -> BTreeSet<String> {
    listing
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let flags = parts.next()?;
            let name = parts.next()?;
            let is_entry = flags.len() == 6
                && flags.starts_with(['V', 'A', 'S'])
                && name != "="
                && !name.starts_with('-');
            is_entry.then(|| name.to_string())
        })
        .collect()
}

/// Creates a command that never opens a console window on Windows.
pub fn tool_command(path: &Path) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command.stdin(Stdio::null());
    command
}

pub struct CapturedOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Runs a tool to completion with a timeout, capturing its output.
pub fn run_capture<I, S>(path: &Path, args: I, timeout: Duration) -> std::io::Result<CapturedOutput>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut child = tool_command(path)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let out_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let err_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf);
        buf
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "tool did not finish in time",
            ));
        }
        std::thread::sleep(Duration::from_millis(15));
    };
    let stdout = out_thread.join().unwrap_or_default();
    let stderr = err_thread.join().unwrap_or_default();
    Ok(CapturedOutput {
        success: status.success(),
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    })
}

/// FFmpeg/ffprobe argument for an input or output file. The `file:` prefix
/// stops names such as `concat:...` from being read as protocols.
pub fn ffmpeg_file_arg(path: &Path) -> std::ffi::OsString {
    let mut arg = std::ffi::OsString::from("file:");
    arg.push(path.as_os_str());
    arg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_encoder_listing() {
        let listing = "Encoders:\n V..... = Video\n A..... = Audio\n ------\n V....D libx264              libx264 H.264\n A....D aac                  AAC (Advanced Audio Coding)\n A....D libmp3lame           libmp3lame MP3\n";
        let encoders = parse_encoders(listing);
        assert!(encoders.contains("libx264"));
        assert!(encoders.contains("aac"));
        assert!(encoders.contains("libmp3lame"));
        assert!(!encoders.contains("="));
        assert_eq!(encoders.len(), 3);
    }

    #[test]
    fn file_arg_has_protocol_prefix() {
        let arg = ffmpeg_file_arg(Path::new("concat:a|b.mp3"));
        assert_eq!(arg.to_string_lossy(), "file:concat:a|b.mp3");
    }
}
