//! Typed operation requests sent by the UI.

use crate::format::FileFormat;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum OperationId {
    ImageConvert,
    PdfMerge,
    PdfSplit,
    PdfRotate,
    PdfToImages,
    AudioConvert,
    VideoConvert,
    ArchiveCreate,
    ArchiveExtract,
}

/// Simple quality choices shown before any advanced option.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Preset {
    Smallest,
    #[default]
    Balanced,
    Quality,
    /// Archives only: favour speed over size.
    Fastest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "op", rename_all = "camelCase")]
#[ts(export)]
pub enum OperationRequest {
    ImageConvert(ImageConvertOptions),
    PdfMerge(PdfMergeOptions),
    PdfSplit(PdfSplitOptions),
    PdfRotate(PdfRotateOptions),
    PdfToImages(PdfToImagesOptions),
    AudioConvert(AudioConvertOptions),
    VideoConvert(VideoConvertOptions),
    ArchiveCreate(ArchiveCreateOptions),
    ArchiveExtract(ArchiveExtractOptions),
}

impl OperationRequest {
    pub fn id(&self) -> OperationId {
        match self {
            Self::ImageConvert(_) => OperationId::ImageConvert,
            Self::PdfMerge(_) => OperationId::PdfMerge,
            Self::PdfSplit(_) => OperationId::PdfSplit,
            Self::PdfRotate(_) => OperationId::PdfRotate,
            Self::PdfToImages(_) => OperationId::PdfToImages,
            Self::AudioConvert(_) => OperationId::AudioConvert,
            Self::VideoConvert(_) => OperationId::VideoConvert,
            Self::ArchiveCreate(_) => OperationId::ArchiveCreate,
            Self::ArchiveExtract(_) => OperationId::ArchiveExtract,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImageConvertOptions {
    pub target: FileFormat,
    pub preset: Preset,
    /// Overrides the preset quality (1-100) for lossy formats.
    pub quality: Option<u8>,
    /// WebP only: keep every pixel exactly.
    #[serde(default)]
    pub lossless: bool,
    pub resize: Option<ResizeSpec>,
    /// Background used when transparency must be removed, as `#rrggbb`.
    pub background: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "mode", rename_all = "camelCase")]
#[ts(export)]
pub enum ResizeSpec {
    /// Fit inside the box, keeping proportions. Never enlarges.
    #[serde(rename_all = "camelCase")]
    Fit {
        max_width: Option<u32>,
        max_height: Option<u32>,
    },
    /// Scale both sides by a percentage.
    Percent { percent: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PdfMergeOptions {
    pub output_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PdfSplitOptions {
    /// `None` creates one PDF per page; otherwise one PDF per range.
    pub ranges: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PdfRotateOptions {
    /// Clockwise: 90, 180 or 270.
    pub angle: u16,
    pub pages: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PdfToImagesOptions {
    pub target: FileFormat,
    pub preset: Preset,
    pub pages: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AudioConvertOptions {
    pub target: FileFormat,
    pub preset: Preset,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VideoConvertOptions {
    pub target: FileFormat,
    pub preset: Preset,
    /// Shrinks taller videos to this height; never enlarges.
    pub max_height: Option<u32>,
    #[serde(default)]
    pub remove_audio: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ArchiveCreateOptions {
    pub target: FileFormat,
    pub preset: Preset,
    pub output_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ArchiveExtractOptions {}

/// Where results are written.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum OutputLocation {
    /// Next to each original file.
    #[default]
    NextToSource,
    /// A folder the user picked through the native dialog.
    Folder { path: String },
}

/// What to do when the output name is already taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CollisionPolicy {
    /// Keep both: `photo (1).webp`.
    #[default]
    Rename,
    /// Replace the existing file. Never applies to the files being converted.
    Overwrite,
    /// Leave the existing file and skip this item.
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OutputSettings {
    pub location: OutputLocation,
    pub collision: CollisionPolicy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_json_shape() {
        let json = r##"{"op":"imageConvert","target":"webp","preset":"smallest","quality":null,"lossless":false,"resize":{"mode":"fit","maxWidth":1920,"maxHeight":null},"background":"#ffffff"}"##;
        let request: OperationRequest = serde_json::from_str(json).unwrap();
        let OperationRequest::ImageConvert(options) = &request else {
            panic!("wrong variant")
        };
        assert_eq!(options.target, FileFormat::Webp);
        assert_eq!(
            options.resize,
            Some(ResizeSpec::Fit {
                max_width: Some(1920),
                max_height: None
            })
        );
        assert_eq!(request.id(), OperationId::ImageConvert);
        let back = serde_json::to_string(&request).unwrap();
        let again: OperationRequest = serde_json::from_str(&back).unwrap();
        assert_eq!(again, request);
    }

    #[test]
    fn output_settings_shape() {
        let json = r#"{"location":{"kind":"folder","path":"/tmp/out"},"collision":"skip"}"#;
        let settings: OutputSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.collision, CollisionPolicy::Skip);
        assert_eq!(
            settings.location,
            OutputLocation::Folder {
                path: "/tmp/out".into()
            }
        );
    }
}
