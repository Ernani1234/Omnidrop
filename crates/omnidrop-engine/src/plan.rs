//! OperationPlanner: checks a request against the selected files and turns it
//! into concrete tasks with resolved output locations.

use crate::capability::{CapabilityRegistry, ConversionCapability, InputMode};
use crate::error::{EngineError, EngineResult, ErrorCode};
use crate::format::{FileFormat, FormatFamily};
use crate::inspector::{FileDetails, FileInfo};
use crate::names::{sanitize_file_name, stem_of};
use crate::operation::{
    ImageConvertOptions, OperationId, OperationRequest, OutputLocation, OutputSettings, ResizeSpec,
};
use crate::pages;
use serde::Serialize;
use std::path::{Path, PathBuf};
use ts_rs::TS;

/// Largest side accepted when resizing.
pub const MAX_RESIZE_SIDE: u32 = 20_000;
pub const MAX_RESIZE_PERCENT: u32 = 400;

/// Informational notes about how a result differs from the original.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "code", rename_all = "camelCase")]
#[ts(export)]
pub enum TaskNote {
    /// Transparent areas were filled with the background colour.
    TransparencyFlattened,
    /// Only the first frame of an animation was converted.
    FirstFrameOnly,
    /// The embedded colour profile could not be kept.
    ColorProfileDropped,
    /// Only the audio track of a video is used.
    AudioFromVideo,
    /// Entries with unsafe paths or links were not extracted.
    #[serde(rename_all = "camelCase")]
    UnsafeEntriesSkipped { count: u32 },
    /// Pages were rendered at a lower resolution to respect size limits.
    ResolutionReduced,
}

#[derive(Debug, Clone)]
pub struct TaskInput {
    pub file_id: String,
    pub path: PathBuf,
    pub name: String,
    pub relative_dir: Option<String>,
    pub format: Option<FileFormat>,
    pub details: Option<FileDetails>,
    pub size: u64,
}

impl TaskInput {
    fn from_info(info: &FileInfo) -> Self {
        Self {
            file_id: info.id.clone(),
            path: info.path.clone(),
            name: info.name.clone(),
            relative_dir: info.relative_dir.clone(),
            format: info.format,
            details: info.details.clone(),
            size: info.size,
        }
    }

    pub fn page_count(&self) -> Option<u32> {
        match self.details {
            Some(FileDetails::Pdf { pages }) => Some(pages),
            _ => None,
        }
    }
}

/// Where a task's result should land, before collision handling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputTarget {
    pub dir: PathBuf,
    pub name: String,
    pub is_dir: bool,
}

impl OutputTarget {
    pub fn desired_path(&self) -> PathBuf {
        self.dir.join(&self.name)
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskSpec {
    pub id: String,
    pub file_ids: Vec<String>,
    /// File or result name shown in the queue.
    pub label: String,
    /// Proposed output name (before collision handling).
    pub output_name: String,
    pub notes: Vec<TaskNote>,
    #[serde(skip)]
    pub inputs: Vec<TaskInput>,
    #[serde(skip)]
    pub output: Option<OutputTarget>,
}

impl TaskSpec {
    pub fn output(&self) -> &OutputTarget {
        self.output
            .as_ref()
            .expect("planned tasks always have an output")
    }
}

/// A file left out of the plan, with the reason.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlanIssue {
    pub file_id: String,
    pub name: String,
    pub error: EngineError,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Plan {
    pub job_id: String,
    pub operation: OperationId,
    pub request: OperationRequest,
    pub output_settings: OutputSettings,
    pub tasks: Vec<TaskSpec>,
    pub skipped: Vec<PlanIssue>,
    /// Number of files that will be written, when it can be known exactly.
    pub expected_outputs: Option<u32>,
}

pub fn plan(
    files: &[FileInfo],
    request: &OperationRequest,
    settings: &OutputSettings,
    registry: &CapabilityRegistry,
) -> EngineResult<Plan> {
    let capability = registry.get(request.id());
    if !capability.available {
        return Err(EngineError::new(
            capability
                .unavailable_reason
                .unwrap_or(ErrorCode::UnsupportedConversion),
            format!("{:?} is not available on this computer", capability.id),
        ));
    }
    validate_options(request, capability)?;
    if let OutputLocation::Folder { path } = &settings.location
        && path.trim().is_empty()
    {
        return Err(EngineError::invalid_options("output folder is empty"));
    }

    let mut accepted: Vec<&FileInfo> = Vec::new();
    let mut skipped = Vec::new();
    for file in files {
        match check_file(file, request, capability) {
            Ok(()) => accepted.push(file),
            Err(error) => skipped.push(PlanIssue {
                file_id: file.id.clone(),
                name: file.name.clone(),
                error,
            }),
        }
    }

    let tasks = match capability.input_mode {
        InputMode::PerFile => accepted
            .iter()
            .map(|file| per_file_task(file, request, settings))
            .collect::<Vec<_>>(),
        InputMode::Combine { min_files } => {
            if accepted.len() < min_files as usize {
                return Err(EngineError::new(
                    ErrorCode::NotEnoughFiles,
                    format!("needs at least {min_files} compatible files"),
                ));
            }
            vec![combined_task(&accepted, request, settings)]
        }
    };
    let expected_outputs = expected_outputs(request, &tasks);

    Ok(Plan {
        job_id: uuid::Uuid::new_v4().to_string(),
        operation: request.id(),
        request: request.clone(),
        output_settings: settings.clone(),
        tasks,
        skipped,
        expected_outputs,
    })
}

fn validate_options(
    request: &OperationRequest,
    capability: &ConversionCapability,
) -> EngineResult<()> {
    let check_target = |target: FileFormat| {
        if capability.produces(target) {
            Ok(())
        } else {
            Err(EngineError::new(
                ErrorCode::UnsupportedConversion,
                format!("cannot produce {target:?} with {:?}", capability.id),
            ))
        }
    };
    match request {
        OperationRequest::ImageConvert(options) => {
            check_target(options.target)?;
            validate_image_options(options)
        }
        OperationRequest::PdfToImages(options) => check_target(options.target),
        OperationRequest::PdfRotate(options) => match options.angle {
            90 | 180 | 270 => Ok(()),
            other => Err(EngineError::invalid_options(format!(
                "angle {other} is not 90, 180 or 270"
            ))),
        },
        OperationRequest::AudioConvert(options) => check_target(options.target),
        OperationRequest::VideoConvert(options) => {
            check_target(options.target)?;
            match options.max_height {
                Some(height) if !(144..=4320).contains(&height) => Err(
                    EngineError::invalid_options(format!("max height {height} out of range")),
                ),
                _ => Ok(()),
            }
        }
        OperationRequest::ArchiveCreate(options) => check_target(options.target),
        OperationRequest::PdfMerge(_)
        | OperationRequest::PdfSplit(_)
        | OperationRequest::ArchiveExtract(_) => Ok(()),
    }
}

fn validate_image_options(options: &ImageConvertOptions) -> EngineResult<()> {
    if let Some(quality) = options.quality
        && !(1..=100).contains(&quality)
    {
        return Err(EngineError::invalid_options(format!(
            "quality {quality} out of 1-100"
        )));
    }
    match options.resize {
        Some(ResizeSpec::Fit {
            max_width,
            max_height,
        }) => {
            if max_width.is_none() && max_height.is_none() {
                return Err(EngineError::invalid_options(
                    "resize needs a width or a height",
                ));
            }
            for side in [max_width, max_height].into_iter().flatten() {
                if !(1..=MAX_RESIZE_SIDE).contains(&side) {
                    return Err(EngineError::invalid_options(format!(
                        "size {side} out of range"
                    )));
                }
            }
        }
        Some(ResizeSpec::Percent { percent }) if !(1..=MAX_RESIZE_PERCENT).contains(&percent) => {
            return Err(EngineError::invalid_options(format!(
                "percent {percent} out of range"
            )));
        }
        _ => {}
    }
    if let Some(color) = &options.background {
        parse_hex_color(color)
            .ok_or_else(|| EngineError::invalid_options(format!("invalid colour {color}")))?;
    }
    Ok(())
}

/// Parses `#rrggbb` (or `rrggbb`).
pub fn parse_hex_color(raw: &str) -> Option<[u8; 3]> {
    let hex = raw.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?])
}

fn check_file(
    file: &FileInfo,
    request: &OperationRequest,
    capability: &ConversionCapability,
) -> EngineResult<()> {
    if let Some(issue) = &file.issue {
        return Err(issue.clone());
    }
    if !capability.accepts(file.format) {
        return Err(EngineError::new(
            if file.format.is_none() {
                ErrorCode::UnsupportedFormat
            } else {
                ErrorCode::UnsupportedConversion
            },
            format!("{:?} does not accept {:?}", capability.id, file.format),
        ));
    }
    if file.size > capability.max_input_bytes {
        return Err(EngineError::new(
            ErrorCode::InputTooLarge,
            "file exceeds size limit",
        ));
    }
    let page_count = || match file.details {
        Some(FileDetails::Pdf { pages }) => Ok(pages),
        _ => Err(EngineError::new(
            ErrorCode::Unreadable,
            "page count unknown",
        )),
    };
    match request {
        OperationRequest::PdfSplit(options) => {
            if let Some(ranges) = &options.ranges {
                pages::parse_ranges(ranges, page_count()?)?;
            }
        }
        OperationRequest::PdfRotate(options) => {
            pages::parse_ranges(options.pages.as_deref().unwrap_or(""), page_count()?)?;
        }
        OperationRequest::PdfToImages(options) => {
            pages::parse_ranges(options.pages.as_deref().unwrap_or(""), page_count()?)?;
        }
        OperationRequest::AudioConvert(_) => {
            if let Some(FileDetails::Media {
                has_audio: false, ..
            }) = file.details
            {
                return Err(EngineError::new(
                    ErrorCode::UnsupportedConversion,
                    "no audio track",
                ));
            }
        }
        OperationRequest::VideoConvert(_) => {
            if let Some(FileDetails::Media {
                has_video: false, ..
            }) = file.details
            {
                return Err(EngineError::new(
                    ErrorCode::UnsupportedConversion,
                    "no video track",
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

fn output_dir(file: &FileInfo, settings: &OutputSettings) -> PathBuf {
    match &settings.location {
        OutputLocation::NextToSource => file
            .path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(".")),
        OutputLocation::Folder { path } => {
            let mut dir = PathBuf::from(path);
            if let Some(relative) = &file.relative_dir {
                for part in relative.split('/').filter(|p| !p.is_empty() && *p != "..") {
                    dir.push(sanitize_file_name(part, "_"));
                }
            }
            dir
        }
    }
}

fn per_file_task(
    file: &FileInfo,
    request: &OperationRequest,
    settings: &OutputSettings,
) -> TaskSpec {
    let stem = stem_of(&file.path);
    let (name, is_dir) = match request {
        OperationRequest::ImageConvert(o) => (format!("{stem}.{}", o.target.extension()), false),
        OperationRequest::PdfRotate(_) => (format!("{stem}.pdf"), false),
        OperationRequest::AudioConvert(o) => (format!("{stem}.{}", o.target.extension()), false),
        OperationRequest::VideoConvert(o) => (format!("{stem}.{}", o.target.extension()), false),
        OperationRequest::PdfSplit(_)
        | OperationRequest::PdfToImages(_)
        | OperationRequest::ArchiveExtract(_) => (stem.clone(), true),
        OperationRequest::PdfMerge(_) | OperationRequest::ArchiveCreate(_) => {
            unreachable!("combined operations are planned by combined_task")
        }
    };
    let mut notes = Vec::new();
    if let (
        OperationRequest::ImageConvert(o),
        Some(FileDetails::Image {
            has_alpha,
            animated,
            ..
        }),
    ) = (request, &file.details)
    {
        if *has_alpha && !o.target.supports_alpha() {
            notes.push(TaskNote::TransparencyFlattened);
        }
        if *animated {
            notes.push(TaskNote::FirstFrameOnly);
        }
    }
    if matches!(request, OperationRequest::AudioConvert(_))
        && file.family == Some(FormatFamily::Video)
    {
        notes.push(TaskNote::AudioFromVideo);
    }
    TaskSpec {
        id: uuid::Uuid::new_v4().to_string(),
        file_ids: vec![file.id.clone()],
        label: file.name.clone(),
        output_name: name.clone(),
        notes,
        inputs: vec![TaskInput::from_info(file)],
        output: Some(OutputTarget {
            dir: output_dir(file, settings),
            name,
            is_dir,
        }),
    }
}

fn combined_task(
    files: &[&FileInfo],
    request: &OperationRequest,
    settings: &OutputSettings,
) -> TaskSpec {
    let (base, extension) = match request {
        OperationRequest::PdfMerge(o) => (o.output_name.as_deref(), "pdf"),
        OperationRequest::ArchiveCreate(o) => (o.output_name.as_deref(), o.target.extension()),
        _ => unreachable!("only merge and archive creation combine files"),
    };
    let fallback = match request {
        OperationRequest::PdfMerge(_) => "merged".to_string(),
        _ if files.len() == 1 => stem_of(&files[0].path),
        _ => common_folder(files).unwrap_or_else(|| "archive".to_string()),
    };
    let stem = sanitize_file_name(base.unwrap_or(&fallback), &fallback);
    let name = format!("{stem}.{extension}");
    // Combined results go next to the first file, or at the root of the
    // chosen folder (not inside a mirrored subfolder).
    let dir = match &settings.location {
        OutputLocation::NextToSource => output_dir(files[0], settings),
        OutputLocation::Folder { path } => PathBuf::from(path),
    };
    TaskSpec {
        id: uuid::Uuid::new_v4().to_string(),
        file_ids: files.iter().map(|f| f.id.clone()).collect(),
        label: name.clone(),
        output_name: name.clone(),
        notes: Vec::new(),
        inputs: files.iter().map(|f| TaskInput::from_info(f)).collect(),
        output: Some(OutputTarget {
            dir,
            name,
            is_dir: false,
        }),
    }
}

/// Name of the imported folder shared by every file, if any.
fn common_folder(files: &[&FileInfo]) -> Option<String> {
    let first = files[0].relative_dir.as_deref()?.split('/').next()?;
    files
        .iter()
        .all(|f| f.relative_dir.as_deref().and_then(|r| r.split('/').next()) == Some(first))
        .then(|| first.to_string())
}

fn expected_outputs(request: &OperationRequest, tasks: &[TaskSpec]) -> Option<u32> {
    let pages_of = |task: &TaskSpec, spec: Option<&str>| -> Option<u32> {
        let count = task.inputs[0].page_count()?;
        pages::selected_pages(spec.unwrap_or(""), count)
            .ok()
            .map(|p| p.len() as u32)
    };
    match request {
        OperationRequest::ImageConvert(_)
        | OperationRequest::PdfRotate(_)
        | OperationRequest::AudioConvert(_)
        | OperationRequest::VideoConvert(_) => Some(tasks.len() as u32),
        OperationRequest::PdfMerge(_) | OperationRequest::ArchiveCreate(_) => Some(1),
        OperationRequest::PdfSplit(o) => tasks.iter().try_fold(0u32, |acc, task| {
            let count = task.inputs[0].page_count()?;
            let groups = match &o.ranges {
                Some(spec) => pages::parse_ranges(spec, count).ok()?.len() as u32,
                None => count,
            };
            Some(acc + groups)
        }),
        OperationRequest::PdfToImages(o) => tasks.iter().try_fold(0u32, |acc, task| {
            Some(acc + pages_of(task, o.pages.as_deref())?)
        }),
        OperationRequest::ArchiveExtract(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::*;
    use crate::tools::ToolSet;

    fn file(
        id: &str,
        path: &str,
        format: Option<FileFormat>,
        details: Option<FileDetails>,
    ) -> FileInfo {
        FileInfo {
            id: id.into(),
            path: PathBuf::from(path),
            name: Path::new(path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            display_path: path.into(),
            relative_dir: None,
            size: 100,
            format,
            family: format.map(FileFormat::family),
            extension: None,
            extension_mismatch: false,
            details,
            issue: None,
        }
    }

    fn image(id: &str, path: &str, has_alpha: bool) -> FileInfo {
        file(
            id,
            path,
            Some(FileFormat::Png),
            Some(FileDetails::Image {
                width: 10,
                height: 10,
                has_alpha,
                animated: false,
            }),
        )
    }

    fn webp_request() -> OperationRequest {
        OperationRequest::ImageConvert(ImageConvertOptions {
            target: FileFormat::Jpeg,
            preset: Preset::Balanced,
            quality: None,
            lossless: false,
            resize: None,
            background: None,
        })
    }

    #[test]
    fn plans_per_file_and_skips_incompatible() {
        let registry = CapabilityRegistry::new(&ToolSet::none());
        let files = vec![
            image("a", "/in/a.png", true),
            file(
                "b",
                "/in/b.pdf",
                Some(FileFormat::Pdf),
                Some(FileDetails::Pdf { pages: 2 }),
            ),
            file("c", "/in/c.bin", None, None),
        ];
        let plan = plan(
            &files,
            &webp_request(),
            &OutputSettings::default(),
            &registry,
        )
        .unwrap();
        assert_eq!(plan.tasks.len(), 1);
        let task = &plan.tasks[0];
        assert_eq!(
            task.output().desired_path(),
            PathBuf::from("/in").join("a.jpg")
        );
        assert_eq!(task.notes, vec![TaskNote::TransparencyFlattened]);
        assert_eq!(plan.skipped.len(), 2);
        assert_eq!(plan.skipped[0].error.code, ErrorCode::UnsupportedConversion);
        assert_eq!(plan.skipped[1].error.code, ErrorCode::UnsupportedFormat);
        assert_eq!(plan.expected_outputs, Some(1));
    }

    #[test]
    fn mirrors_folders_in_custom_output() {
        let registry = CapabilityRegistry::new(&ToolSet::none());
        let mut f = image("a", "/in/Photos/2024/a.png", false);
        f.relative_dir = Some("Photos/2024".into());
        let settings = OutputSettings {
            location: OutputLocation::Folder {
                path: "/out".into(),
            },
            collision: CollisionPolicy::Rename,
        };
        let plan = plan(&[f], &webp_request(), &settings, &registry).unwrap();
        assert_eq!(
            plan.tasks[0].output().desired_path(),
            PathBuf::from("/out")
                .join("Photos")
                .join("2024")
                .join("a.jpg")
        );
    }

    #[test]
    fn rejects_unavailable_and_invalid_requests() {
        let registry = CapabilityRegistry::new(&ToolSet::none());
        let audio = OperationRequest::AudioConvert(AudioConvertOptions {
            target: FileFormat::Mp3,
            preset: Preset::Balanced,
        });
        let err = plan(&[], &audio, &OutputSettings::default(), &registry).unwrap_err();
        assert_eq!(err.code, ErrorCode::ToolMissing);

        let gif_output = OperationRequest::ImageConvert(ImageConvertOptions {
            target: FileFormat::Gif,
            preset: Preset::Balanced,
            quality: None,
            lossless: false,
            resize: None,
            background: None,
        });
        let err = plan(&[], &gif_output, &OutputSettings::default(), &registry).unwrap_err();
        assert_eq!(err.code, ErrorCode::UnsupportedConversion);

        let bad_quality = OperationRequest::ImageConvert(ImageConvertOptions {
            target: FileFormat::Jpeg,
            preset: Preset::Balanced,
            quality: Some(0),
            lossless: false,
            resize: Some(ResizeSpec::Percent { percent: 50 }),
            background: None,
        });
        let err = plan(&[], &bad_quality, &OutputSettings::default(), &registry).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidOptions);

        let bad_angle = OperationRequest::PdfRotate(PdfRotateOptions {
            angle: 45,
            pages: None,
        });
        assert!(plan(&[], &bad_angle, &OutputSettings::default(), &registry).is_err());
    }

    #[test]
    fn merge_needs_two_pdfs_and_counts_outputs() {
        let registry = CapabilityRegistry::new(&ToolSet::none());
        let pdf = |id: &str, pages| {
            file(
                id,
                &format!("/docs/{id}.pdf"),
                Some(FileFormat::Pdf),
                Some(FileDetails::Pdf { pages }),
            )
        };
        let merge = OperationRequest::PdfMerge(PdfMergeOptions {
            output_name: Some("Relatório: final".into()),
        });
        let err = plan(
            &[pdf("a", 1)],
            &merge,
            &OutputSettings::default(),
            &registry,
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::NotEnoughFiles);

        let p = plan(
            &[pdf("a", 1), pdf("b", 3)],
            &merge,
            &OutputSettings::default(),
            &registry,
        )
        .unwrap();
        assert_eq!(p.tasks.len(), 1);
        assert_eq!(p.tasks[0].inputs.len(), 2);
        assert_eq!(p.tasks[0].output().name, "Relatório_ final.pdf");

        let split = OperationRequest::PdfSplit(PdfSplitOptions {
            ranges: Some("1, 2-3".into()),
        });
        let p = plan(
            &[pdf("a", 1), pdf("b", 3)],
            &split,
            &OutputSettings::default(),
            &registry,
        )
        .unwrap();
        // "2-3" is out of range for a one-page file.
        assert_eq!(p.tasks.len(), 1);
        assert_eq!(p.skipped[0].error.code, ErrorCode::InvalidPageRange);
        assert_eq!(p.expected_outputs, Some(2));
        assert!(p.tasks[0].output().is_dir);

        let to_images = OperationRequest::PdfToImages(PdfToImagesOptions {
            target: FileFormat::Png,
            preset: Preset::Balanced,
            pages: None,
        });
        let p = plan(
            &[pdf("a", 1), pdf("b", 3)],
            &to_images,
            &OutputSettings::default(),
            &registry,
        )
        .unwrap();
        assert_eq!(p.expected_outputs, Some(4));
    }

    #[test]
    fn archive_names_default_to_common_folder() {
        let registry = CapabilityRegistry::new(&ToolSet::none());
        let mut a = file("a", "/x/Trip/a.txt", None, None);
        a.relative_dir = Some("Trip".into());
        let mut b = file("b", "/x/Trip/sub/b.txt", None, None);
        b.relative_dir = Some("Trip/sub".into());
        let request = OperationRequest::ArchiveCreate(ArchiveCreateOptions {
            target: FileFormat::Zip,
            preset: Preset::Balanced,
            output_name: None,
        });
        let p = plan(&[a, b], &request, &OutputSettings::default(), &registry).unwrap();
        assert_eq!(p.tasks[0].output().name, "Trip.zip");
    }

    #[test]
    fn inspection_issues_are_skipped() {
        let registry = CapabilityRegistry::new(&ToolSet::none());
        let mut broken = image("a", "/in/a.png", false);
        broken.issue = Some(EngineError::new(ErrorCode::Unreadable, "corrupt"));
        let p = plan(
            &[broken],
            &webp_request(),
            &OutputSettings::default(),
            &registry,
        )
        .unwrap();
        assert!(p.tasks.is_empty());
        assert_eq!(p.skipped[0].error.code, ErrorCode::Unreadable);
    }

    #[test]
    fn parses_colors() {
        assert_eq!(parse_hex_color("#ff8000"), Some([255, 128, 0]));
        assert_eq!(parse_hex_color("FFFFFF"), Some([255, 255, 255]));
        assert_eq!(parse_hex_color("#fff"), None);
        assert_eq!(parse_hex_color("#gggggg"), None);
    }
}
