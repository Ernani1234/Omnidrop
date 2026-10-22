//! OutputValidator: confirms a staged result exists, can be read back and
//! matches what the operation promised, before it is published.

use crate::error::{EngineError, EngineResult, ErrorCode};
use crate::format::{self, FileFormat, SNIFF_LEN};
use crate::inspector::FileDetails;
use crate::tools::ToolSet;
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum Expectation {
    Image {
        format: FileFormat,
        width: u32,
        height: u32,
    },
    Pdf {
        pages: u32,
    },
    Archive {
        format: FileFormat,
        entries: u64,
    },
    Media {
        format: FileFormat,
        has_video: bool,
        /// `None` when the input's audio state is unknown.
        has_audio: Option<bool>,
    },
    Folder {
        files: u64,
        each: Option<FolderItem>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FolderItem {
    Pdf,
    Image(FileFormat),
}

pub fn validate(path: &Path, expectation: &Expectation, tools: &ToolSet) -> EngineResult<()> {
    match expectation {
        Expectation::Folder { files, each } => validate_folder(path, *files, *each),
        _ => {
            let meta = fs::metadata(path).map_err(|e| failed(format!("missing output: {e}")))?;
            if !meta.is_file() || meta.len() == 0 {
                return Err(failed("output is empty"));
            }
            match expectation {
                Expectation::Image {
                    format,
                    width,
                    height,
                } => validate_image(path, *format, Some((*width, *height))),
                Expectation::Pdf { pages } => validate_pdf(path, Some(*pages)),
                Expectation::Archive { format, entries } => {
                    validate_archive(path, *format, *entries)
                }
                Expectation::Media {
                    format,
                    has_video,
                    has_audio,
                } => validate_media(path, *format, *has_video, *has_audio, tools),
                Expectation::Folder { .. } => unreachable!(),
            }
        }
    }
}

fn failed(detail: impl Into<String>) -> EngineError {
    EngineError::new(ErrorCode::ValidationFailed, detail)
}

fn sniff_file(path: &Path) -> EngineResult<Option<FileFormat>> {
    let file = File::open(path).map_err(|e| failed(e.to_string()))?;
    let mut header = Vec::with_capacity(SNIFF_LEN);
    file.take(SNIFF_LEN as u64)
        .read_to_end(&mut header)
        .map_err(|e| failed(e.to_string()))?;
    let mut sniffed = format::sniff(&header);
    if sniffed.is_none() && format::is_gzip(&header) {
        sniffed = Some(FileFormat::TarGz);
    }
    Ok(sniffed)
}

fn validate_image(path: &Path, expected: FileFormat, size: Option<(u32, u32)>) -> EngineResult<()> {
    let detected = sniff_file(path)?;
    if detected != Some(expected) {
        return Err(failed(format!("expected {expected:?}, found {detected:?}")));
    }
    let dimensions = if expected == FileFormat::Avif {
        // AVIF cannot be decoded here; check the declared size instead.
        let mut header = Vec::new();
        File::open(path)
            .and_then(|f| f.take(SNIFF_LEN as u64).read_to_end(&mut header))
            .map_err(|e| failed(e.to_string()))?;
        avif_ispe(&header)
    } else {
        Some(
            image::ImageReader::open(path)
                .map_err(|e| failed(e.to_string()))?
                .with_guessed_format()
                .map_err(|e| failed(e.to_string()))?
                .into_dimensions()
                .map_err(|e| failed(e.to_string()))?,
        )
    };
    if let (Some(expected), Some(actual)) = (size, dimensions)
        && expected != actual
    {
        return Err(failed(format!(
            "expected {expected:?} pixels, found {actual:?}"
        )));
    }
    Ok(())
}

fn avif_ispe(header: &[u8]) -> Option<(u32, u32)> {
    let pos = header.windows(4).position(|w| w == b"ispe")?;
    let data = header.get(pos + 8..pos + 16)?;
    Some((
        u32::from_be_bytes(data[0..4].try_into().ok()?),
        u32::from_be_bytes(data[4..8].try_into().ok()?),
    ))
}

fn validate_pdf(path: &Path, pages: Option<u32>) -> EngineResult<()> {
    let doc = lopdf::Document::load(path).map_err(|e| failed(format!("unreadable PDF: {e}")))?;
    let count = doc.get_pages().len() as u32;
    if count == 0 {
        return Err(failed("PDF has no pages"));
    }
    if let Some(expected) = pages
        && expected != count
    {
        return Err(failed(format!("expected {expected} pages, found {count}")));
    }
    Ok(())
}

fn validate_archive(path: &Path, format: FileFormat, entries: u64) -> EngineResult<()> {
    let file = File::open(path).map_err(|e| failed(e.to_string()))?;
    let count = match format {
        FileFormat::Zip => zip::ZipArchive::new(BufReader::new(file))
            .map_err(|e| failed(e.to_string()))?
            .len() as u64,
        FileFormat::SevenZ => {
            sevenz_rust2::ArchiveReader::new(BufReader::new(file), sevenz_rust2::Password::empty())
                .map_err(|e| failed(e.to_string()))?
                .archive()
                .files
                .len() as u64
        }
        FileFormat::Tar => count_tar(BufReader::new(file))?,
        FileFormat::TarGz => count_tar(flate2::read::GzDecoder::new(BufReader::new(file)))?,
        other => return Err(failed(format!("{other:?} is not an archive"))),
    };
    if count != entries {
        return Err(failed(format!("expected {entries} entries, found {count}")));
    }
    Ok(())
}

fn count_tar<R: Read>(reader: R) -> EngineResult<u64> {
    let mut archive = tar::Archive::new(reader);
    let mut count = 0;
    for entry in archive.entries().map_err(|e| failed(e.to_string()))? {
        let mut entry = entry.map_err(|e| failed(e.to_string()))?;
        // Read the data so truncated archives are caught.
        std::io::copy(&mut entry, &mut std::io::sink()).map_err(|e| failed(e.to_string()))?;
        count += 1;
    }
    Ok(count)
}

fn validate_media(
    path: &Path,
    format: FileFormat,
    has_video: bool,
    has_audio: Option<bool>,
    tools: &ToolSet,
) -> EngineResult<()> {
    let Some(ffprobe) = tools.ffprobe.as_ref() else {
        let detected = sniff_file(path)?;
        return if detected == Some(format) {
            Ok(())
        } else {
            Err(failed(format!("expected {format:?}, found {detected:?}")))
        };
    };
    let details = crate::executors::media::probe(&ffprobe.executable, path)
        .map_err(|e| failed(format!("output cannot be read: {}", e.detail)))?;
    let FileDetails::Media {
        has_video: video,
        has_audio: audio,
        duration_ms,
        ..
    } = details
    else {
        return Err(failed("unexpected probe result"));
    };
    if video != has_video {
        return Err(failed(format!(
            "video stream expected: {has_video}, found: {video}"
        )));
    }
    if let Some(expected) = has_audio
        && expected != audio
    {
        return Err(failed(format!(
            "audio stream expected: {expected}, found: {audio}"
        )));
    }
    if duration_ms == Some(0) {
        return Err(failed("output has no duration"));
    }
    Ok(())
}

fn validate_folder(path: &Path, files: u64, each: Option<FolderItem>) -> EngineResult<()> {
    let mut found = Vec::new();
    collect_files(path, &mut found).map_err(|e| failed(e.to_string()))?;
    if found.len() as u64 != files {
        return Err(failed(format!(
            "expected {files} files, found {}",
            found.len()
        )));
    }
    for file in &found {
        match each {
            Some(FolderItem::Pdf) => validate_pdf(file, None)?,
            Some(FolderItem::Image(format)) => validate_image(file, format, None)?,
            None => {}
        }
    }
    Ok(())
}

fn collect_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect_files(&entry.path(), out)?;
        } else if kind.is_file() {
            out.push(entry.path());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_empty_and_wrong_format() {
        let dir = tempfile::tempdir().unwrap();
        let tools = ToolSet::none();
        let png = Expectation::Image {
            format: FileFormat::Png,
            width: 2,
            height: 2,
        };
        let missing = dir.path().join("missing.png");
        assert_eq!(
            validate(&missing, &png, &tools).unwrap_err().code,
            ErrorCode::ValidationFailed
        );

        let empty = dir.path().join("empty.png");
        fs::write(&empty, b"").unwrap();
        assert!(validate(&empty, &png, &tools).is_err());

        let jpeg = dir.path().join("actually.png");
        image::RgbImage::new(2, 2)
            .save_with_format(&jpeg, image::ImageFormat::Jpeg)
            .unwrap();
        assert!(validate(&jpeg, &png, &tools).is_err());

        let good = dir.path().join("good.png");
        image::RgbImage::new(2, 2)
            .save_with_format(&good, image::ImageFormat::Png)
            .unwrap();
        validate(&good, &png, &tools).unwrap();
        let wrong_size = Expectation::Image {
            format: FileFormat::Png,
            width: 3,
            height: 2,
        };
        assert!(validate(&good, &wrong_size, &tools).is_err());
    }

    #[test]
    fn folder_counts_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("a.txt"), b"a").unwrap();
        fs::write(dir.path().join("sub").join("b.txt"), b"b").unwrap();
        let tools = ToolSet::none();
        validate(
            dir.path(),
            &Expectation::Folder {
                files: 2,
                each: None,
            },
            &tools,
        )
        .unwrap();
        assert!(
            validate(
                dir.path(),
                &Expectation::Folder {
                    files: 3,
                    each: None
                },
                &tools
            )
            .is_err()
        );
    }
}
