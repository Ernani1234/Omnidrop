//! File-name sanitization and safe path handling.

use std::path::{Component, Path, PathBuf};

const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

const MAX_NAME_CHARS: usize = 180;

/// Turns arbitrary text into a file-name component that is valid on Windows,
/// macOS and Linux. Returns `fallback` when nothing usable is left.
pub fn sanitize_file_name(raw: &str, fallback: &str) -> String {
    let mut name: String = raw
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    name = name.trim().trim_end_matches(['.', ' ']).to_string();
    if name.chars().count() > MAX_NAME_CHARS {
        name = name.chars().take(MAX_NAME_CHARS).collect();
        name = name.trim_end_matches(['.', ' ']).to_string();
    }
    if name.is_empty() || name == "." || name == ".." {
        return fallback.to_string();
    }
    let stem_upper = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if WINDOWS_RESERVED.contains(&stem_upper.as_str()) {
        name.insert(0, '_');
    }
    name
}

/// File stem of `path`, sanitized; `.tar.gz` is stripped as a whole.
pub fn stem_of(path: &Path) -> String {
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let lower = file_name.to_ascii_lowercase();
    let stem = if lower.ends_with(".tar.gz") {
        file_name[..file_name.len() - 7].to_string()
    } else {
        path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    sanitize_file_name(&stem, "file")
}

/// `name.ext` → `name (n).ext`; directories have no extension.
pub fn numbered_candidate(desired: &Path, n: u32) -> PathBuf {
    let parent = desired.parent().unwrap_or_else(|| Path::new(""));
    let file_name = desired
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (stem, ext) = split_name(&file_name);
    let candidate = match ext {
        Some(ext) => format!("{stem} ({n}).{ext}"),
        None => format!("{stem} ({n})"),
    };
    parent.join(candidate)
}

fn split_name(file_name: &str) -> (&str, Option<&str>) {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".tar.gz") && file_name.len() > 7 {
        let cut = file_name.len() - 7;
        return (&file_name[..cut], Some(&file_name[cut + 1..]));
    }
    match file_name.rfind('.') {
        Some(idx) if idx > 0 => (&file_name[..idx], Some(&file_name[idx + 1..])),
        _ => (file_name, None),
    }
}

/// Validates an archive entry name and converts it to a relative path that
/// stays inside the extraction folder.
///
/// Rejects absolute paths, drive prefixes and any `..` component (instead of
/// silently rewriting them), and sanitizes each component for the host OS.
pub fn safe_relative_path(entry_name: &str) -> Option<PathBuf> {
    if entry_name.contains('\0') {
        return None;
    }
    let normalized = entry_name.replace('\\', "/");
    if normalized.starts_with('/') {
        return None;
    }
    let mut out = PathBuf::new();
    for component in Path::new(&normalized).components() {
        match component {
            Component::Normal(part) => {
                let part = part.to_string_lossy();
                // A drive-letter or stream marker smuggled into a component.
                if part.contains(':') && cfg!(windows) {
                    return None;
                }
                let clean = sanitize_file_name(&part, "_");
                out.push(clean);
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Archive entry name for a file: forward slashes, sanitized components.
pub fn archive_entry_name(relative_dir: Option<&str>, file_name: &str) -> String {
    let mut parts: Vec<String> = relative_dir
        .unwrap_or_default()
        .split(['/', '\\'])
        .filter(|p| !p.is_empty() && *p != "." && *p != "..")
        .map(|p| sanitize_file_name(p, "_"))
        .collect();
    parts.push(sanitize_file_name(file_name, "file"));
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_names() {
        assert_eq!(sanitize_file_name("a/b:c*d?.png", "x"), "a_b_c_d_.png");
        assert_eq!(sanitize_file_name("  report. ", "x"), "report");
        assert_eq!(sanitize_file_name("", "fallback"), "fallback");
        assert_eq!(sanitize_file_name("..", "fallback"), "fallback");
        assert_eq!(sanitize_file_name("CON.txt", "x"), "_CON.txt");
        assert_eq!(sanitize_file_name("con", "x"), "_con");
        let long = "a".repeat(400);
        assert_eq!(
            sanitize_file_name(&long, "x").chars().count(),
            MAX_NAME_CHARS
        );
    }

    #[test]
    fn numbered_candidates() {
        let p = Path::new("dir").join("photo.webp");
        assert_eq!(
            numbered_candidate(&p, 2),
            Path::new("dir").join("photo (2).webp")
        );
        let d = Path::new("dir").join("Archive");
        assert_eq!(
            numbered_candidate(&d, 1),
            Path::new("dir").join("Archive (1)")
        );
        let t = Path::new("backup.tar.gz");
        assert_eq!(numbered_candidate(t, 1), PathBuf::from("backup (1).tar.gz"));
        let hidden = Path::new(".env");
        assert_eq!(numbered_candidate(hidden, 1), PathBuf::from(".env (1)"));
    }

    #[test]
    fn stems() {
        assert_eq!(stem_of(Path::new("/x/photo.final.PNG")), "photo.final");
        assert_eq!(stem_of(Path::new("/x/backup.tar.gz")), "backup");
        assert_eq!(stem_of(Path::new("/x/noext")), "noext");
    }

    #[test]
    fn rejects_traversal_entries() {
        assert_eq!(safe_relative_path("../evil.txt"), None);
        assert_eq!(safe_relative_path("a/../../evil.txt"), None);
        assert_eq!(safe_relative_path("/etc/passwd"), None);
        assert_eq!(safe_relative_path("\\windows\\system32"), None);
        assert_eq!(safe_relative_path("..\\..\\evil"), None);
        assert_eq!(safe_relative_path(""), None);
        assert_eq!(safe_relative_path("./"), None);
        assert_eq!(safe_relative_path("a\0b"), None);
        #[cfg(windows)]
        assert_eq!(safe_relative_path("C:/evil"), None);
    }

    #[test]
    fn accepts_nested_entries() {
        assert_eq!(
            safe_relative_path("docs/./img/a.png"),
            Some(Path::new("docs").join("img").join("a.png"))
        );
        assert_eq!(
            safe_relative_path("docs\\b.txt"),
            Some(Path::new("docs").join("b.txt"))
        );
    }

    #[test]
    fn archive_entry_names() {
        assert_eq!(
            archive_entry_name(Some("Fotos/2024"), "a.png"),
            "Fotos/2024/a.png"
        );
        assert_eq!(archive_entry_name(Some("../x"), "a.png"), "x/a.png");
        assert_eq!(archive_entry_name(None, "a.png"), "a.png");
    }
}
