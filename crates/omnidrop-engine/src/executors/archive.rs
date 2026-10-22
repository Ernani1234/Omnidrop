//! Archives: create and extract ZIP, TAR, TAR.GZ and 7z.
//!
//! Extraction never follows the paths stored in the archive blindly: entries
//! that would escape the destination, links and special files are skipped,
//! and size, count and expansion-ratio limits stop decompression bombs.

use super::Produced;
use crate::cancel::{TaskCancel, is_cancel_error};
use crate::error::{EngineError, EngineResult, ErrorCode};
use crate::format::FileFormat;
use crate::job::TaskContext;
use crate::limits::Limits;
use crate::names::{archive_entry_name, numbered_candidate, safe_relative_path};
use crate::operation::{ArchiveCreateOptions, Preset};
use crate::output::Staged;
use crate::plan::{TaskInput, TaskNote, TaskSpec};
use crate::validator::Expectation;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const CHUNK: usize = 256 * 1024;

/// Reader that reports bytes read and stops on cancellation.
struct ProgressReader<'a, R> {
    inner: R,
    cancel: &'a TaskCancel,
    on_read: &'a mut dyn FnMut(u64),
}

impl<R: Read> Read for ProgressReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.cancel.is_cancelled() {
            return Err(io::Error::other("omnidrop: cancelled"));
        }
        let n = self.inner.read(buf)?;
        (self.on_read)(n as u64);
        Ok(n)
    }
}

fn io_failure(err: io::Error, writing: bool) -> EngineError {
    if is_cancel_error(&err) {
        EngineError::cancelled()
    } else if writing {
        EngineError::write(&err)
    } else {
        EngineError::read(&err)
    }
}

/// Gives each input a unique entry name inside the archive.
fn entry_names(inputs: &[TaskInput]) -> Vec<String> {
    let mut used = HashSet::new();
    inputs
        .iter()
        .map(|input| {
            let base = archive_entry_name(input.relative_dir.as_deref(), &input.name);
            let mut name = base.clone();
            let mut n = 1;
            while !used.insert(name.to_lowercase()) {
                name = numbered_candidate(Path::new(&base), n)
                    .to_string_lossy()
                    .replace('\\', "/");
                n += 1;
            }
            name
        })
        .collect()
}

fn compression_level(preset: Preset, format: FileFormat) -> u32 {
    match (format, preset) {
        (FileFormat::SevenZ, Preset::Smallest) => 9,
        (FileFormat::SevenZ, Preset::Fastest) => 1,
        (FileFormat::SevenZ, _) => 5,
        (_, Preset::Smallest) => 9,
        (_, Preset::Fastest) => 1,
        (_, _) => 6,
    }
}

pub fn create(
    options: &ArchiveCreateOptions,
    task: &TaskSpec,
    ctx: &TaskContext<'_>,
) -> EngineResult<Produced> {
    let names = entry_names(&task.inputs);
    let total: u64 = task.inputs.iter().map(|i| i.size).sum::<u64>().max(1);
    let mut done = 0u64;
    let mut on_read = |n: u64| {
        done += n;
        ctx.progress(0.9 * done as f32 / total as f32);
    };
    let level = compression_level(options.preset, options.target);
    let mut temp = ctx.output.temp_file(&task.output().dir)?;
    {
        let file = temp.as_file_mut();
        match options.target {
            FileFormat::Zip => write_zip(file, task, &names, level, &ctx.cancel, &mut on_read)?,
            FileFormat::Tar => {
                let writer = BufWriter::new(file);
                let writer = write_tar(writer, task, &names, &ctx.cancel, &mut on_read)?;
                finish_writer(writer)?;
            }
            FileFormat::TarGz => {
                let gz = flate2::write::GzEncoder::new(
                    BufWriter::new(file),
                    flate2::Compression::new(level),
                );
                let gz = write_tar(gz, task, &names, &ctx.cancel, &mut on_read)?;
                let writer = gz.finish().map_err(|e| io_failure(e, true))?;
                finish_writer(writer)?;
            }
            FileFormat::SevenZ => write_7z(file, task, &names, level, &ctx.cancel, &mut on_read)?,
            other => {
                return Err(EngineError::new(
                    ErrorCode::UnsupportedConversion,
                    format!("cannot create {other:?} archives"),
                ));
            }
        }
    }
    Ok(Produced {
        staged: Staged::File(temp),
        expectation: Expectation::Archive {
            format: options.target,
            entries: names.len() as u64,
        },
        notes: Vec::new(),
    })
}

fn finish_writer<W: Write>(mut writer: BufWriter<W>) -> EngineResult<()> {
    writer.flush().map_err(|e| io_failure(e, true))
}

fn open_input(input: &TaskInput) -> EngineResult<File> {
    File::open(&input.path).map_err(|e| EngineError::read(&e))
}

fn zip_time(time: Option<SystemTime>) -> zip::DateTime {
    let Some(secs) = time
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
    else {
        return zip::DateTime::default();
    };
    let (year, month, day) = civil_from_days(secs.div_euclid(86_400));
    let rem = secs.rem_euclid(86_400);
    zip::DateTime::from_date_and_time(
        year.clamp(1980, 2107) as u16,
        month as u8,
        day as u8,
        (rem / 3600) as u8,
        ((rem % 3600) / 60) as u8,
        (rem % 60) as u8,
    )
    .unwrap_or_default()
}

/// Days since 1970-01-01 to (year, month, day). Howard Hinnant's algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn write_zip(
    file: &mut File,
    task: &TaskSpec,
    names: &[String],
    level: u32,
    cancel: &TaskCancel,
    on_read: &mut dyn FnMut(u64),
) -> EngineResult<()> {
    let mut zip = zip::ZipWriter::new(BufWriter::new(file));
    let zip_err = |e: zip::result::ZipError| match e {
        zip::result::ZipError::Io(io) => io_failure(io, true),
        other => EngineError::internal(format!("zip writer: {other}")),
    };
    for (input, name) in task.inputs.iter().zip(names) {
        cancel.check()?;
        let source = open_input(input)?;
        let meta = source.metadata().map_err(|e| EngineError::read(&e))?;
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(i64::from(level)))
            .large_file(meta.len() >= u64::from(u32::MAX) / 2)
            .last_modified_time(zip_time(meta.modified().ok()))
            .unix_permissions(0o644);
        zip.start_file(name.as_str(), options).map_err(zip_err)?;
        let mut reader = ProgressReader {
            inner: BufReader::with_capacity(CHUNK, source),
            cancel,
            on_read: &mut *on_read,
        };
        io::copy(&mut reader, &mut zip).map_err(|e| io_failure(e, true))?;
    }
    let writer = zip.finish().map_err(zip_err)?;
    finish_writer(writer)
}

fn write_tar<W: Write>(
    writer: W,
    task: &TaskSpec,
    names: &[String],
    cancel: &TaskCancel,
    on_read: &mut dyn FnMut(u64),
) -> EngineResult<W> {
    let mut builder = tar::Builder::new(writer);
    for (input, name) in task.inputs.iter().zip(names) {
        cancel.check()?;
        let source = open_input(input)?;
        let meta = source.metadata().map_err(|e| EngineError::read(&e))?;
        let mut header = tar::Header::new_gnu();
        header.set_metadata_in_mode(&meta, tar::HeaderMode::Complete);
        header.set_mode(0o644);
        header.set_size(meta.len());
        let reader = ProgressReader {
            inner: BufReader::with_capacity(CHUNK, source),
            cancel,
            on_read: &mut *on_read,
        };
        builder
            .append_data(&mut header, name, reader)
            .map_err(|e| io_failure(e, true))?;
    }
    builder.into_inner().map_err(|e| io_failure(e, true))
}

fn write_7z(
    file: &mut File,
    task: &TaskSpec,
    names: &[String],
    level: u32,
    cancel: &TaskCancel,
    on_read: &mut dyn FnMut(u64),
) -> EngineResult<()> {
    use sevenz_rust2::encoder_options::Lzma2Options;
    let seven_err = |e: sevenz_rust2::Error| match e {
        sevenz_rust2::Error::Io(io, _) => io_failure(io, true),
        other => EngineError::internal(format!("7z writer: {other}")),
    };
    let mut writer = sevenz_rust2::ArchiveWriter::new(BufWriter::new(file)).map_err(seven_err)?;
    writer.set_content_methods(vec![Lzma2Options::from_level(level).into()]);
    for (input, name) in task.inputs.iter().zip(names) {
        cancel.check()?;
        let source = open_input(input)?;
        let entry = sevenz_rust2::ArchiveEntry::from_path(&input.path, name.clone());
        let reader = ProgressReader {
            inner: BufReader::with_capacity(CHUNK, source),
            cancel,
            on_read: &mut *on_read,
        };
        writer
            .push_archive_entry(entry, Some(reader))
            .map_err(seven_err)?;
    }
    let inner = writer.finish().map_err(|e| io_failure(e, true))?;
    finish_writer(inner)
}

/// Tracks extraction against the configured limits.
struct ExtractGuard<'a> {
    limits: &'a Limits,
    archive_size: u64,
    entries: u64,
    written: u64,
}

impl ExtractGuard<'_> {
    fn entry(&mut self) -> EngineResult<()> {
        self.entries += 1;
        if self.entries > self.limits.archive_max_entries {
            return Err(EngineError::new(
                ErrorCode::ArchiveLimitExceeded,
                "too many entries",
            ));
        }
        Ok(())
    }

    fn bytes(&mut self, n: u64) -> EngineResult<()> {
        self.written += n;
        if self.written > self.limits.archive_max_total_bytes {
            return Err(EngineError::new(
                ErrorCode::ArchiveLimitExceeded,
                "archive expands beyond the size limit",
            ));
        }
        let ratio = self.written / self.archive_size.max(1);
        if self.written > self.limits.archive_ratio_grace_bytes
            && ratio > self.limits.archive_max_ratio
        {
            return Err(EngineError::new(
                ErrorCode::ArchiveLimitExceeded,
                format!("expansion ratio {ratio}:1 looks like a decompression bomb"),
            ));
        }
        Ok(())
    }
}

struct Extraction<'a, 'c> {
    root: PathBuf,
    guard: ExtractGuard<'a>,
    ctx: &'a TaskContext<'c>,
    files: u64,
    skipped: u32,
}

impl Extraction<'_, '_> {
    /// Copies one entry, enforcing limits as bytes arrive (declared sizes
    /// in headers cannot be trusted).
    fn write_file(&mut self, relative: &Path, reader: &mut dyn Read) -> EngineResult<()> {
        self.guard.entry()?;
        let dest = self.root.join(relative);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| EngineError::write(&e))?;
        }
        if dest.is_dir() {
            // Conflicting entry (a folder already uses this name).
            self.skipped += 1;
            return self.drain(reader);
        }
        let mut out = BufWriter::new(File::create(&dest).map_err(|e| EngineError::write(&e))?);
        let mut buf = vec![0u8; CHUNK];
        loop {
            self.ctx.cancel.check()?;
            let n = match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    return Err(EngineError::new(
                        ErrorCode::Unreadable,
                        format!("corrupt entry: {e}"),
                    ));
                }
            };
            self.guard.bytes(n as u64)?;
            out.write_all(&buf[..n])
                .map_err(|e| EngineError::write(&e))?;
        }
        out.flush().map_err(|e| EngineError::write(&e))?;
        self.files += 1;
        Ok(())
    }

    fn make_dir(&mut self, relative: &Path) -> EngineResult<()> {
        let dest = self.root.join(relative);
        if dest.is_file() {
            self.skipped += 1;
            return Ok(());
        }
        fs::create_dir_all(dest).map_err(|e| EngineError::write(&e))
    }

    /// Reads and discards a skipped entry; still counted against limits.
    fn drain(&mut self, reader: &mut dyn Read) -> EngineResult<()> {
        let mut buf = vec![0u8; CHUNK];
        loop {
            self.ctx.cancel.check()?;
            match reader.read(&mut buf) {
                Ok(0) => return Ok(()),
                Ok(n) => self.guard.bytes(n as u64)?,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => {
                    return Err(EngineError::new(
                        ErrorCode::Unreadable,
                        format!("corrupt entry: {e}"),
                    ));
                }
            }
        }
    }
}

pub fn extract(task: &TaskSpec, ctx: &TaskContext<'_>) -> EngineResult<Produced> {
    let input = &task.inputs[0];
    let format = input
        .format
        .ok_or_else(|| EngineError::new(ErrorCode::UnsupportedFormat, "unknown archive"))?;
    let staging = ctx.output.temp_dir(&task.output().dir)?;
    let mut extraction = Extraction {
        root: staging.path().to_path_buf(),
        guard: ExtractGuard {
            limits: ctx.limits,
            archive_size: input.size,
            entries: 0,
            written: 0,
        },
        ctx,
        files: 0,
        skipped: 0,
    };
    match format {
        FileFormat::Zip => extract_zip(&input.path, &mut extraction)?,
        FileFormat::Tar | FileFormat::TarGz => {
            let size = input.size.max(1);
            let file = File::open(&input.path).map_err(|e| EngineError::read(&e))?;
            let mut read = 0u64;
            let mut on_read = |n: u64| {
                read += n;
                ctx.progress(0.9 * read as f32 / size as f32);
            };
            let counted = ProgressReader {
                inner: BufReader::with_capacity(CHUNK, file),
                cancel: &ctx.cancel,
                on_read: &mut on_read,
            };
            if format == FileFormat::TarGz {
                extract_tar(flate2::read::GzDecoder::new(counted), &mut extraction)?;
            } else {
                extract_tar(counted, &mut extraction)?;
            }
        }
        FileFormat::SevenZ => extract_7z(&input.path, &mut extraction)?,
        other => {
            return Err(EngineError::new(
                ErrorCode::UnsupportedConversion,
                format!("cannot extract {other:?}"),
            ));
        }
    }
    if extraction.files == 0 {
        return Err(EngineError::new(
            ErrorCode::Unreadable,
            "archive has no files that can be extracted",
        ));
    }
    let mut notes = Vec::new();
    if extraction.skipped > 0 {
        notes.push(TaskNote::UnsafeEntriesSkipped {
            count: extraction.skipped,
        });
    }
    let files = extraction.files;
    Ok(Produced {
        staged: Staged::Dir(staging),
        expectation: Expectation::Folder { files, each: None },
        notes,
    })
}

fn extract_zip(path: &Path, extraction: &mut Extraction<'_, '_>) -> EngineResult<()> {
    let file = File::open(path).map_err(|e| EngineError::read(&e))?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file))
        .map_err(|e| EngineError::new(ErrorCode::Unreadable, e.to_string()))?;
    let count = archive.len();
    for index in 0..count {
        extraction.ctx.cancel.check()?;
        let mut entry = match archive.by_index(index) {
            Ok(entry) => entry,
            Err(err) => {
                let text = err.to_string();
                let code = if text.to_lowercase().contains("password") || text.contains("encrypt") {
                    ErrorCode::ArchiveEncrypted
                } else {
                    ErrorCode::Unreadable
                };
                return Err(EngineError::new(code, text));
            }
        };
        if entry.encrypted() {
            return Err(EngineError::new(
                ErrorCode::ArchiveEncrypted,
                "encrypted entry",
            ));
        }
        let name = entry.name().map(|n| n.into_owned()).unwrap_or_default();
        let relative = safe_relative_path(&name);
        match relative {
            Some(rel) if entry.is_dir() => extraction.make_dir(&rel)?,
            Some(rel) if entry.is_file() => extraction.write_file(&rel, &mut entry)?,
            _ => extraction.skipped += 1,
        }
        extraction
            .ctx
            .progress(0.9 * (index + 1) as f32 / count.max(1) as f32);
    }
    Ok(())
}

fn extract_tar<R: Read>(reader: R, extraction: &mut Extraction<'_, '_>) -> EngineResult<()> {
    let mut archive = tar::Archive::new(reader);
    let entries = archive.entries().map_err(|e| io_failure(e, false))?;
    for entry in entries {
        extraction.ctx.cancel.check()?;
        let mut entry = entry.map_err(|e| {
            if is_cancel_error(&e) {
                EngineError::cancelled()
            } else {
                EngineError::new(ErrorCode::Unreadable, e.to_string())
            }
        })?;
        let name = String::from_utf8_lossy(&entry.path_bytes()).into_owned();
        let relative = safe_relative_path(&name);
        let kind = entry.header().entry_type();
        match relative {
            Some(rel) if kind.is_dir() => extraction.make_dir(&rel)?,
            Some(rel) if kind.is_file() || kind == tar::EntryType::Continuous => {
                extraction.write_file(&rel, &mut entry)?
            }
            // Links, devices and unsafe paths.
            _ => {
                extraction.skipped += 1;
                extraction.drain(&mut entry)?;
            }
        }
    }
    Ok(())
}

const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const UNIX_EXTENSION: u32 = 0x8000;
const S_IFMT: u32 = 0o170_000;
const S_IFLNK: u32 = 0o120_000;

fn is_7z_link(entry: &sevenz_rust2::ArchiveEntry) -> bool {
    let attributes = entry.windows_attributes();
    if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return true;
    }
    attributes & UNIX_EXTENSION != 0 && ((attributes >> 16) & S_IFMT) == S_IFLNK
}

pub fn is_password_error(err: &sevenz_rust2::Error) -> bool {
    matches!(
        err,
        sevenz_rust2::Error::PasswordRequired | sevenz_rust2::Error::MaybeBadPassword(_)
    )
}

fn extract_7z_entry(
    extraction: &mut Extraction<'_, '_>,
    entry: &sevenz_rust2::ArchiveEntry,
    data: &mut dyn Read,
) -> EngineResult<()> {
    match safe_relative_path(entry.name()) {
        Some(rel) if entry.is_directory() => extraction.make_dir(&rel),
        Some(rel) if !is_7z_link(entry) => extraction.write_file(&rel, data),
        _ => {
            extraction.skipped += 1;
            extraction.drain(data)
        }
    }
}

fn extract_7z(path: &Path, extraction: &mut Extraction<'_, '_>) -> EngineResult<()> {
    let file = File::open(path).map_err(|e| EngineError::read(&e))?;
    let mut reader =
        sevenz_rust2::ArchiveReader::new(BufReader::new(file), sevenz_rust2::Password::empty())
            .map_err(|e| {
                if is_password_error(&e) {
                    EngineError::new(ErrorCode::ArchiveEncrypted, e.to_string())
                } else {
                    EngineError::new(ErrorCode::Unreadable, e.to_string())
                }
            })?;
    let total = reader.archive().files.len().max(1);
    let mut processed = 0usize;
    let mut failure: Option<EngineError> = None;
    let result = reader.for_each_entries(|entry, data| {
        let step = extract_7z_entry(extraction, entry, data);
        processed += 1;
        extraction
            .ctx
            .progress(0.9 * processed as f32 / total as f32);
        match step {
            Ok(()) => Ok(true),
            Err(err) => {
                failure = Some(err);
                Err(io::Error::other("omnidrop: stop").into())
            }
        }
    });
    if let Some(err) = failure {
        return Err(err);
    }
    result.map_err(|e| {
        if is_password_error(&e) {
            EngineError::new(ErrorCode::ArchiveEncrypted, e.to_string())
        } else {
            EngineError::new(ErrorCode::Unreadable, e.to_string())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    }

    #[test]
    fn unique_entry_names() {
        let input = |name: &str, dir: Option<&str>| TaskInput {
            file_id: name.into(),
            path: PathBuf::from(name),
            name: name.into(),
            relative_dir: dir.map(String::from),
            format: None,
            details: None,
            size: 0,
        };
        let names = entry_names(&[
            input("a.txt", None),
            input("A.txt", None),
            input("a.txt", Some("docs")),
        ]);
        assert_eq!(names, vec!["a.txt", "A (1).txt", "docs/a.txt"]);
    }
}
