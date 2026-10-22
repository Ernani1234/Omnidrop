//! Declares which operations exist, what they accept and produce, and whether
//! they can run on this machine. The UI only offers what is listed here.

use crate::error::ErrorCode;
use crate::format::{FileFormat, FormatFamily};
use crate::operation::{OperationId, Preset};
use crate::tools::ToolSet;
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ExternalTool {
    Ffmpeg,
}

/// How an operation consumes the selected files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum InputMode {
    /// One task per file.
    PerFile,
    /// All compatible files become one result.
    #[serde(rename_all = "camelCase")]
    Combine { min_files: u32 },
}

/// How quickly a running task reacts to cancellation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CancelSupport {
    /// Stops as soon as requested.
    Immediate,
    /// Stops at the next step (page, file, stage); queued items stop at once.
    BetweenSteps,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConversionCapability {
    pub id: OperationId,
    pub family: FormatFamily,
    pub input_formats: Vec<FileFormat>,
    /// Accepts any file, including unrecognized ones (archives).
    pub accepts_any_input: bool,
    /// Formats that can be produced on this machine right now.
    pub output_formats: Vec<FileFormat>,
    /// Formats the operation knows but cannot produce here (missing codec).
    pub unavailable_output_formats: Vec<FileFormat>,
    pub input_mode: InputMode,
    pub supports_batch: bool,
    pub supports_quality_settings: bool,
    pub cancellation: CancelSupport,
    pub requires_external_binary: Option<ExternalTool>,
    pub available: bool,
    pub unavailable_reason: Option<ErrorCode>,
    pub presets: Vec<Preset>,
    /// Largest accepted input, in bytes.
    #[ts(type = "number")]
    pub max_input_bytes: u64,
}

impl ConversionCapability {
    pub fn accepts(&self, format: Option<FileFormat>) -> bool {
        if self.accepts_any_input {
            return true;
        }
        format.is_some_and(|f| self.input_formats.contains(&f))
    }

    pub fn produces(&self, format: FileFormat) -> bool {
        self.output_formats.contains(&format)
    }
}

pub const IMAGE_INPUTS: &[FileFormat] = &[
    FileFormat::Png,
    FileFormat::Jpeg,
    FileFormat::Webp,
    FileFormat::Tiff,
    FileFormat::Bmp,
    FileFormat::Gif,
    FileFormat::Ico,
];

pub const IMAGE_OUTPUTS: &[FileFormat] = &[
    FileFormat::Png,
    FileFormat::Jpeg,
    FileFormat::Webp,
    FileFormat::Avif,
    FileFormat::Tiff,
    FileFormat::Bmp,
];

pub const PDF_IMAGE_OUTPUTS: &[FileFormat] = &[FileFormat::Png, FileFormat::Jpeg, FileFormat::Webp];

pub const AUDIO_INPUTS: &[FileFormat] = &[
    FileFormat::Mp3,
    FileFormat::Wav,
    FileFormat::Flac,
    FileFormat::Aac,
    FileFormat::M4a,
    FileFormat::Ogg,
    FileFormat::Mp4,
    FileFormat::Mov,
    FileFormat::Mkv,
    FileFormat::Webm,
    FileFormat::Avi,
];

pub const VIDEO_INPUTS: &[FileFormat] = &[
    FileFormat::Mp4,
    FileFormat::Mov,
    FileFormat::Mkv,
    FileFormat::Webm,
    FileFormat::Avi,
];

pub const ARCHIVE_FORMATS: &[FileFormat] = &[
    FileFormat::Zip,
    FileFormat::Tar,
    FileFormat::TarGz,
    FileFormat::SevenZ,
];

/// FFmpeg encoders each media output needs.
pub fn media_encoders(format: FileFormat) -> &'static [&'static [&'static str]] {
    // Each inner slice lists alternatives; every group must be satisfied.
    match format {
        FileFormat::Mp3 => &[&["libmp3lame"]],
        FileFormat::M4a => &[&["aac"]],
        FileFormat::Flac => &[&["flac"]],
        FileFormat::Wav => &[&["pcm_s16le"]],
        FileFormat::Mp4 | FileFormat::Mkv => &[&["libx264"], &["aac"]],
        FileFormat::Webm => &[&["libvpx-vp9"], &["libopus", "libvorbis"]],
        _ => &[],
    }
}

const GIB: u64 = 1024 * 1024 * 1024;
const QUALITY_PRESETS: [Preset; 3] = [Preset::Smallest, Preset::Balanced, Preset::Quality];

#[derive(Debug, Clone)]
pub struct CapabilityRegistry {
    capabilities: Vec<ConversionCapability>,
}

impl CapabilityRegistry {
    pub fn new(tools: &ToolSet) -> Self {
        let mut capabilities = vec![
            ConversionCapability {
                id: OperationId::ImageConvert,
                family: FormatFamily::Image,
                input_formats: IMAGE_INPUTS.to_vec(),
                accepts_any_input: false,
                output_formats: IMAGE_OUTPUTS.to_vec(),
                unavailable_output_formats: vec![],
                input_mode: InputMode::PerFile,
                supports_batch: true,
                supports_quality_settings: true,
                cancellation: CancelSupport::BetweenSteps,
                requires_external_binary: None,
                available: true,
                unavailable_reason: None,
                presets: QUALITY_PRESETS.to_vec(),
                max_input_bytes: 2 * GIB,
            },
            pdf_capability(
                OperationId::PdfMerge,
                vec![],
                InputMode::Combine { min_files: 2 },
            ),
            pdf_capability(OperationId::PdfSplit, vec![], InputMode::PerFile),
            pdf_capability(OperationId::PdfRotate, vec![], InputMode::PerFile),
            ConversionCapability {
                supports_quality_settings: true,
                presets: QUALITY_PRESETS.to_vec(),
                ..pdf_capability(
                    OperationId::PdfToImages,
                    PDF_IMAGE_OUTPUTS.to_vec(),
                    InputMode::PerFile,
                )
            },
        ];

        let audio_outputs = [
            FileFormat::Mp3,
            FileFormat::M4a,
            FileFormat::Flac,
            FileFormat::Wav,
        ];
        let video_outputs = [FileFormat::Mp4, FileFormat::Webm, FileFormat::Mkv];
        capabilities.push(media_capability(
            OperationId::AudioConvert,
            FormatFamily::Audio,
            AUDIO_INPUTS,
            &audio_outputs,
            tools,
        ));
        capabilities.push(media_capability(
            OperationId::VideoConvert,
            FormatFamily::Video,
            VIDEO_INPUTS,
            &video_outputs,
            tools,
        ));

        capabilities.push(ConversionCapability {
            id: OperationId::ArchiveCreate,
            family: FormatFamily::Archive,
            input_formats: vec![],
            accepts_any_input: true,
            output_formats: ARCHIVE_FORMATS.to_vec(),
            unavailable_output_formats: vec![],
            input_mode: InputMode::Combine { min_files: 1 },
            supports_batch: true,
            supports_quality_settings: true,
            cancellation: CancelSupport::BetweenSteps,
            requires_external_binary: None,
            available: true,
            unavailable_reason: None,
            presets: vec![Preset::Smallest, Preset::Balanced, Preset::Fastest],
            max_input_bytes: 50 * GIB,
        });
        capabilities.push(ConversionCapability {
            id: OperationId::ArchiveExtract,
            family: FormatFamily::Archive,
            input_formats: ARCHIVE_FORMATS.to_vec(),
            accepts_any_input: false,
            output_formats: vec![],
            unavailable_output_formats: vec![],
            input_mode: InputMode::PerFile,
            supports_batch: true,
            supports_quality_settings: false,
            cancellation: CancelSupport::BetweenSteps,
            requires_external_binary: None,
            available: true,
            unavailable_reason: None,
            presets: vec![],
            max_input_bytes: 50 * GIB,
        });
        Self { capabilities }
    }

    pub fn list(&self) -> &[ConversionCapability] {
        &self.capabilities
    }

    pub fn get(&self, id: OperationId) -> &ConversionCapability {
        self.capabilities
            .iter()
            .find(|c| c.id == id)
            .expect("every OperationId is registered")
    }

    /// Operations that can run on at least one of `formats`.
    pub fn operations_for(&self, format: Option<FileFormat>) -> Vec<OperationId> {
        self.capabilities
            .iter()
            .filter(|c| c.available && c.accepts(format))
            .map(|c| c.id)
            .collect()
    }
}

fn pdf_capability(
    id: OperationId,
    outputs: Vec<FileFormat>,
    input_mode: InputMode,
) -> ConversionCapability {
    ConversionCapability {
        id,
        family: FormatFamily::Pdf,
        input_formats: vec![FileFormat::Pdf],
        accepts_any_input: false,
        output_formats: outputs,
        unavailable_output_formats: vec![],
        input_mode,
        supports_batch: true,
        supports_quality_settings: false,
        cancellation: CancelSupport::BetweenSteps,
        requires_external_binary: None,
        available: true,
        unavailable_reason: None,
        presets: vec![],
        max_input_bytes: 2 * GIB,
    }
}

fn media_capability(
    id: OperationId,
    family: FormatFamily,
    inputs: &[FileFormat],
    outputs: &[FileFormat],
    tools: &ToolSet,
) -> ConversionCapability {
    let ffmpeg = tools.ffmpeg.as_ref();
    let (available_outputs, missing): (Vec<FileFormat>, Vec<FileFormat>) =
        outputs.iter().partition(|format| {
            ffmpeg.is_some_and(|tool| {
                media_encoders(**format)
                    .iter()
                    .all(|group| group.iter().any(|enc| tool.has_encoder(enc)))
            })
        });
    let (available, reason) = match ffmpeg {
        None => (false, Some(ErrorCode::ToolMissing)),
        Some(_) if available_outputs.is_empty() => (false, Some(ErrorCode::UnsupportedConversion)),
        Some(_) => (true, None),
    };
    ConversionCapability {
        id,
        family,
        input_formats: inputs.to_vec(),
        accepts_any_input: false,
        output_formats: available_outputs,
        unavailable_output_formats: missing,
        input_mode: InputMode::PerFile,
        supports_batch: true,
        supports_quality_settings: true,
        cancellation: CancelSupport::Immediate,
        requires_external_binary: Some(ExternalTool::Ffmpeg),
        available,
        unavailable_reason: reason,
        presets: QUALITY_PRESETS.to_vec(),
        max_input_bytes: 100 * GIB,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolInfo;
    use std::collections::BTreeSet;

    fn fake_ffmpeg(encoders: &[&str]) -> ToolSet {
        ToolSet {
            ffmpeg: Some(ToolInfo {
                path: "ffmpeg".into(),
                version: "test".into(),
                executable: "ffmpeg".into(),
                encoders: encoders
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<BTreeSet<_>>(),
            }),
            ffprobe: None,
        }
    }

    #[test]
    fn media_is_unavailable_without_ffmpeg() {
        let registry = CapabilityRegistry::new(&ToolSet::none());
        let audio = registry.get(OperationId::AudioConvert);
        assert!(!audio.available);
        assert_eq!(audio.unavailable_reason, Some(ErrorCode::ToolMissing));
        assert!(audio.output_formats.is_empty());
        assert!(registry.get(OperationId::ImageConvert).available);
    }

    #[test]
    fn media_outputs_follow_encoders() {
        let registry = CapabilityRegistry::new(&fake_ffmpeg(&["aac", "flac", "pcm_s16le"]));
        let audio = registry.get(OperationId::AudioConvert);
        assert!(audio.available);
        assert_eq!(
            audio.output_formats,
            vec![FileFormat::M4a, FileFormat::Flac, FileFormat::Wav]
        );
        assert_eq!(audio.unavailable_output_formats, vec![FileFormat::Mp3]);
        let video = registry.get(OperationId::VideoConvert);
        assert!(!video.available);
        assert_eq!(
            video.unavailable_reason,
            Some(ErrorCode::UnsupportedConversion)
        );
    }

    #[test]
    fn operations_for_formats() {
        let registry = CapabilityRegistry::new(&ToolSet::none());
        let png = registry.operations_for(Some(FileFormat::Png));
        assert!(png.contains(&OperationId::ImageConvert));
        assert!(png.contains(&OperationId::ArchiveCreate));
        assert!(!png.contains(&OperationId::PdfMerge));
        let unknown = registry.operations_for(None);
        assert_eq!(unknown, vec![OperationId::ArchiveCreate]);
        let avif = registry.operations_for(Some(FileFormat::Avif));
        assert!(
            !avif.contains(&OperationId::ImageConvert),
            "AVIF input is not decodable"
        );
    }
}
