//! FileInspector: identifies the real format of each input and reads the
//! metadata the UI shows (dimensions, pages, duration, entries).

use crate::error::{EngineError, ErrorCode};
use crate::format::{self, FileFormat, FormatFamily, SNIFF_LEN};
use crate::limits::Limits;
use crate::tools::ToolSet;
use serde::Serialize;
use std::fs::{self, File};
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileInfo {
    pub id: String,
    /// Authoritative path; never round-tripped through the UI.
    #[serde(skip)]
    pub path: PathBuf,
    pub name: String,
    pub display_path: String,
    /// Folder path relative to an imported folder, using `/`.
    pub relative_dir: Option<String>,
    #[ts(type = "number")]
    pub size: u64,
    pub format: Option<FileFormat>,
    pub family: Option<FormatFamily>,
    pub extension: Option<String>,
    /// The extension does not match the detected content.
    pub extension_mismatch: bool,
    pub details: Option<FileDetails>,
    /// Problem found while inspecting (unreadable, encrypted, ...).
    pub issue: Option<EngineError>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum FileDetails {
    #[serde(rename_all = "camelCase")]
    Image {
        width: u32,
        height: u32,
        has_alpha: bool,
        animated: bool,
    },
    #[serde(rename_all = "camelCase")]
    Pdf { pages: u32 },
    #[serde(rename_all = "camelCase")]
    Media {
        #[ts(type = "number | null")]
        duration_ms: Option<u64>,
        has_video: bool,
        has_audio: bool,
        width: Option<u32>,
        height: Option<u32>,
        video_codec: Option<String>,
        audio_codec: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Archive {
        #[ts(type = "number")]
        entries: u64,
        #[ts(type = "number | null")]
        uncompressed_bytes: Option<u64>,
    },
}

/// A file to inspect, with its position inside an imported folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputEntry {
    pub path: PathBuf,
    pub relative_dir: Option<String>,
}

#[derive(Debug, Default)]
pub struct ExpandedInputs {
    pub entries: Vec<InputEntry>,
    /// The import stopped at `Limits::max_import_files`.
    pub truncated: bool,
}

const IGNORED_NAMES: &[&str] = &["thumbs.db", "desktop.ini", ".ds_store"];

/// Expands folders into their files (recursively, without following links),
/// skipping hidden and system files.
pub fn expand_inputs(paths: &[PathBuf], limits: &Limits) -> ExpandedInputs {
    let mut out = ExpandedInputs::default();
    for path in paths {
        if out.entries.len() >= limits.max_import_files {
            out.truncated = true;
            break;
        }
        match fs::metadata(path) {
            Ok(meta) if meta.is_dir() => {
                let root_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "folder".into());
                walk_dir(path, &root_name, 0, limits, &mut out);
            }
            // Files, and paths that vanished: inspection reports the problem.
            _ => out.entries.push(InputEntry {
                path: path.clone(),
                relative_dir: None,
            }),
        }
    }
    out
}

fn walk_dir(dir: &Path, relative: &str, depth: usize, limits: &Limits, out: &mut ExpandedInputs) {
    if depth > limits.max_import_depth {
        return;
    }
    let Ok(read) = fs::read_dir(dir) else { return };
    let mut children: Vec<_> = read.filter_map(Result::ok).collect();
    children.sort_by_key(|e| e.file_name());
    for child in children {
        if out.entries.len() >= limits.max_import_files {
            out.truncated = true;
            return;
        }
        let name = child.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || IGNORED_NAMES.contains(&name.to_ascii_lowercase().as_str()) {
            continue;
        }
        let Ok(file_type) = child.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            walk_dir(
                &child.path(),
                &format!("{relative}/{name}"),
                depth + 1,
                limits,
                out,
            );
        } else if file_type.is_file() {
            out.entries.push(InputEntry {
                path: child.path(),
                relative_dir: Some(relative.to_string()),
            });
        }
    }
}

/// Inspects one file. Never fails: problems are reported in `FileInfo::issue`.
pub fn inspect(entry: &InputEntry, id: String, tools: &ToolSet, limits: &Limits) -> FileInfo {
    let path = &entry.path;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());
    let extension = format::extension_of(path);
    let mut info = FileInfo {
        id,
        path: path.clone(),
        name,
        display_path: path.to_string_lossy().into_owned(),
        relative_dir: entry.relative_dir.clone(),
        size: 0,
        format: None,
        family: None,
        extension: extension.clone(),
        extension_mismatch: false,
        details: None,
        issue: None,
    };

    let meta = match fs::metadata(path) {
        Ok(meta) if meta.is_file() => meta,
        Ok(_) => {
            info.issue = Some(EngineError::new(
                ErrorCode::Unreadable,
                "not a regular file",
            ));
            return info;
        }
        Err(err) => {
            info.issue = Some(EngineError::read(&err));
            return info;
        }
    };
    info.size = meta.len();

    let header = match read_header(path) {
        Ok(header) => header,
        Err(err) => {
            info.issue = Some(EngineError::read(&err));
            return info;
        }
    };
    let mut sniffed = format::sniff(&header);
    if sniffed.is_none() && format::is_gzip(&header) && gzip_contains_tar(path) {
        sniffed = Some(FileFormat::TarGz);
    }
    let detected = format::resolve(sniffed, extension.as_deref());
    info.format = detected;
    info.family = detected.map(FileFormat::family);
    info.extension_mismatch = match (detected, extension.as_deref()) {
        (Some(format), Some(ext)) => !format.matches_extension(ext),
        _ => false,
    };

    let Some(detected) = detected else {
        return info;
    };
    let details = match detected.family() {
        FormatFamily::Image => inspect_image(path, detected, &header),
        FormatFamily::Pdf => inspect_pdf(path, info.size, limits),
        FormatFamily::Audio | FormatFamily::Video => match tools.ffprobe.as_ref() {
            Some(probe) => crate::executors::media::probe(&probe.executable, path).map(Some),
            None => Ok(None),
        },
        FormatFamily::Archive => inspect_archive(path, detected),
        FormatFamily::Document => Ok(None),
    };
    match details {
        Ok(details) => info.details = details,
        Err(err) => info.issue = Some(err),
    }
    info
}

fn read_header(path: &Path) -> std::io::Result<Vec<u8>> {
    let file = File::open(path)?;
    let mut header = Vec::with_capacity(SNIFF_LEN);
    file.take(SNIFF_LEN as u64).read_to_end(&mut header)?;
    Ok(header)
}

fn gzip_contains_tar(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    let mut decoder = flate2::read::GzDecoder::new(BufReader::new(file));
    let mut block = [0u8; 512];
    decoder.read_exact(&mut block).is_ok() && &block[257..262] == b"ustar"
}

fn inspect_image(
    path: &Path,
    format: FileFormat,
    header: &[u8],
) -> Result<Option<FileDetails>, EngineError> {
    if format == FileFormat::Avif {
        // No AVIF decoder is bundled; read the size from the container.
        return Ok(
            avif_dimensions(header).map(|(width, height)| FileDetails::Image {
                width,
                height,
                has_alpha: false,
                animated: false,
            }),
        );
    }
    use image::ImageDecoder;
    let reader = image::ImageReader::open(path)
        .map_err(|e| EngineError::read(&e))?
        .with_guessed_format()
        .map_err(|e| EngineError::read(&e))?;
    let decoder = reader
        .into_decoder()
        .map_err(|e| crate::executors::image::map_image_error(&e))?;
    let (width, height) = decoder.dimensions();
    let has_alpha = decoder.color_type().has_alpha();
    let animated = match format {
        FileFormat::Gif => gif_frame_count(path, 2).unwrap_or(1) > 1,
        FileFormat::Png => png_is_animated(header),
        FileFormat::Webp => webp_is_animated(header),
        _ => false,
    };
    Ok(Some(FileDetails::Image {
        width,
        height,
        has_alpha,
        animated,
    }))
}

fn inspect_pdf(
    path: &Path,
    size: u64,
    limits: &Limits,
) -> Result<Option<FileDetails>, EngineError> {
    if size > limits.max_pdf_bytes {
        return Err(EngineError::new(
            ErrorCode::InputTooLarge,
            "PDF exceeds size limit",
        ));
    }
    let data = fs::read(path).map_err(|e| EngineError::read(&e))?;
    let pages = crate::executors::pdf::count_pages(data)?;
    Ok(Some(FileDetails::Pdf { pages }))
}

fn inspect_archive(path: &Path, format: FileFormat) -> Result<Option<FileDetails>, EngineError> {
    let file = File::open(path).map_err(|e| EngineError::read(&e))?;
    let unreadable = |detail: String| EngineError::new(ErrorCode::Unreadable, detail);
    match format {
        FileFormat::Zip => {
            let mut archive = zip::ZipArchive::new(BufReader::new(file))
                .map_err(|e| unreadable(e.to_string()))?;
            let mut total = 0u64;
            for index in 0..archive.len() {
                if let Ok(entry) = archive.by_index_raw(index) {
                    total = total.saturating_add(entry.size());
                }
            }
            Ok(Some(FileDetails::Archive {
                entries: archive.len() as u64,
                uncompressed_bytes: Some(total),
            }))
        }
        FileFormat::SevenZ => {
            let reader = sevenz_rust2::ArchiveReader::new(
                BufReader::new(file),
                sevenz_rust2::Password::empty(),
            );
            match reader {
                Ok(reader) => {
                    let files = &reader.archive().files;
                    Ok(Some(FileDetails::Archive {
                        entries: files.len() as u64,
                        uncompressed_bytes: Some(files.iter().map(|f| f.size()).sum()),
                    }))
                }
                Err(err) if crate::executors::archive::is_password_error(&err) => Err(
                    EngineError::new(ErrorCode::ArchiveEncrypted, err.to_string()),
                ),
                Err(err) => Err(unreadable(err.to_string())),
            }
        }
        FileFormat::Tar => {
            let mut archive = tar::Archive::new(BufReader::new(file));
            let mut entries = 0u64;
            let mut total = 0u64;
            for entry in archive
                .entries_with_seek()
                .map_err(|e| unreadable(e.to_string()))?
            {
                let entry = entry.map_err(|e| unreadable(e.to_string()))?;
                entries += 1;
                total = total.saturating_add(entry.size());
            }
            Ok(Some(FileDetails::Archive {
                entries,
                uncompressed_bytes: Some(total),
            }))
        }
        // Counting .tar.gz entries means decompressing everything; skip.
        _ => Ok(None),
    }
}

/// Reads width/height from the `ispe` property of an AVIF file.
fn avif_dimensions(header: &[u8]) -> Option<(u32, u32)> {
    let pos = header.windows(4).position(|w| w == b"ispe")?;
    let data = header.get(pos + 8..pos + 16)?;
    let width = u32::from_be_bytes(data[0..4].try_into().ok()?);
    let height = u32::from_be_bytes(data[4..8].try_into().ok()?);
    (width > 0 && height > 0).then_some((width, height))
}

/// APNG files carry an `acTL` chunk before the first `IDAT`.
fn png_is_animated(header: &[u8]) -> bool {
    let mut pos = 8;
    while pos + 8 <= header.len() {
        let len = u32::from_be_bytes(header[pos..pos + 4].try_into().unwrap()) as usize;
        let kind = &header[pos + 4..pos + 8];
        if kind == b"acTL" {
            return true;
        }
        if kind == b"IDAT" {
            return false;
        }
        pos = pos.saturating_add(12).saturating_add(len);
    }
    false
}

/// Extended WebP files flag animation in the `VP8X` header.
fn webp_is_animated(header: &[u8]) -> bool {
    header.len() > 20 && &header[12..16] == b"VP8X" && header[20] & 0x02 != 0
}

/// Counts GIF frames, stopping at `stop_at`. Walks the block structure
/// without decoding any pixels.
pub fn gif_frame_count(path: &Path, stop_at: u32) -> std::io::Result<u32> {
    let mut r = BufReader::new(File::open(path)?);
    let mut header = [0u8; 13];
    r.read_exact(&mut header)?;
    if &header[0..3] != b"GIF" {
        return Err(std::io::Error::other("not a GIF"));
    }
    let packed = header[10];
    if packed & 0x80 != 0 {
        let table = 3 * (1u64 << ((packed & 0x07) + 1));
        r.seek(SeekFrom::Current(table as i64))?;
    }
    let mut frames = 0;
    loop {
        let mut byte = [0u8; 1];
        if r.read_exact(&mut byte).is_err() {
            return Ok(frames);
        }
        match byte[0] {
            0x2C => {
                frames += 1;
                if frames >= stop_at {
                    return Ok(frames);
                }
                let mut descriptor = [0u8; 9];
                r.read_exact(&mut descriptor)?;
                let packed = descriptor[8];
                if packed & 0x80 != 0 {
                    let table = 3 * (1u64 << ((packed & 0x07) + 1));
                    r.seek(SeekFrom::Current(table as i64))?;
                }
                let mut lzw_min = [0u8; 1];
                r.read_exact(&mut lzw_min)?;
                skip_sub_blocks(&mut r)?;
            }
            0x21 => {
                let mut label = [0u8; 1];
                r.read_exact(&mut label)?;
                skip_sub_blocks(&mut r)?;
            }
            0x3B => return Ok(frames),
            _ => return Ok(frames),
        }
    }
}

fn skip_sub_blocks<R: Read + Seek>(r: &mut R) -> std::io::Result<()> {
    loop {
        let mut size = [0u8; 1];
        r.read_exact(&mut size)?;
        if size[0] == 0 {
            return Ok(());
        }
        r.seek(SeekFrom::Current(i64::from(size[0])))?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_folders_without_hidden_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Photos");
        fs::create_dir_all(root.join("2024")).unwrap();
        fs::write(root.join("a.png"), b"x").unwrap();
        fs::write(root.join("2024").join("b.png"), b"x").unwrap();
        fs::write(root.join(".hidden"), b"x").unwrap();
        fs::write(root.join("Thumbs.db"), b"x").unwrap();
        let single = dir.path().join("single.txt");
        fs::write(&single, b"x").unwrap();

        let expanded = expand_inputs(&[root.clone(), single.clone()], &Limits::default());
        let rel: Vec<_> = expanded
            .entries
            .iter()
            .map(|e| {
                (
                    e.path.file_name().unwrap().to_string_lossy().into_owned(),
                    e.relative_dir.clone(),
                )
            })
            .collect();
        assert_eq!(
            rel,
            vec![
                ("b.png".to_string(), Some("Photos/2024".to_string())),
                ("a.png".to_string(), Some("Photos".to_string())),
                ("single.txt".to_string(), None),
            ]
        );
        assert!(!expanded.truncated);
    }

    #[test]
    fn import_limit_truncates() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..5 {
            fs::write(dir.path().join(format!("{i}.txt")), b"x").unwrap();
        }
        let limits = Limits {
            max_import_files: 3,
            ..Limits::default()
        };
        let expanded = expand_inputs(&[dir.path().to_path_buf()], &limits);
        assert_eq!(expanded.entries.len(), 3);
        assert!(expanded.truncated);
    }

    #[test]
    fn missing_file_reports_issue() {
        let entry = InputEntry {
            path: PathBuf::from("definitely/missing/file.png"),
            relative_dir: None,
        };
        let info = inspect(&entry, "1".into(), &ToolSet::none(), &Limits::default());
        assert_eq!(info.issue.unwrap().code, ErrorCode::NotFound);
    }

    #[test]
    fn detects_misleading_extension_and_unknown_content() {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("photo.jpg");
        image::RgbImage::new(3, 2)
            .save_with_format(&fake, image::ImageFormat::Png)
            .unwrap();
        let info = inspect(
            &InputEntry {
                path: fake,
                relative_dir: None,
            },
            "1".into(),
            &ToolSet::none(),
            &Limits::default(),
        );
        assert_eq!(info.format, Some(FileFormat::Png));
        assert!(info.extension_mismatch);
        assert_eq!(
            info.details,
            Some(FileDetails::Image {
                width: 3,
                height: 2,
                has_alpha: false,
                animated: false
            })
        );

        let text = dir.path().join("notes.png");
        fs::write(&text, b"just some text that is not an image").unwrap();
        let info = inspect(
            &InputEntry {
                path: text,
                relative_dir: None,
            },
            "2".into(),
            &ToolSet::none(),
            &Limits::default(),
        );
        assert_eq!(info.format, None);
        assert!(info.issue.is_none());
    }

    #[test]
    fn detects_animated_gif() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anim.gif");
        {
            let file = File::create(&path).unwrap();
            let mut encoder = image::codecs::gif::GifEncoder::new(file);
            for shade in [0u8, 255] {
                let frame = image::Frame::new(image::RgbaImage::from_pixel(
                    4,
                    4,
                    image::Rgba([shade, 0, 0, 255]),
                ));
                encoder.encode_frame(frame).unwrap();
            }
        }
        assert_eq!(gif_frame_count(&path, 10).unwrap(), 2);
        let info = inspect(
            &InputEntry {
                path,
                relative_dir: None,
            },
            "1".into(),
            &ToolSet::none(),
            &Limits::default(),
        );
        assert!(matches!(
            info.details,
            Some(FileDetails::Image { animated: true, .. })
        ));
    }

    #[test]
    fn reads_avif_dimensions() {
        let mut header = vec![0u8; 64];
        header[20..24].copy_from_slice(b"ispe");
        header[28..32].copy_from_slice(&640u32.to_be_bytes());
        header[32..36].copy_from_slice(&480u32.to_be_bytes());
        assert_eq!(avif_dimensions(&header), Some((640, 480)));
    }
}
