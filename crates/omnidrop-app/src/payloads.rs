//! Data sent to the UI through commands and events.

use crate::history::HistoryEntry;
use crate::settings::{OutputMode, Settings};
use omnidrop_engine::capability::ConversionCapability;
use omnidrop_engine::{CapabilityRegistry, FileInfo, ToolSet};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use ts_rs::TS;

pub const EVENT_FILES: &str = "omnidrop://files";
pub const EVENT_INSPECTING: &str = "omnidrop://inspecting";
pub const EVENT_DRAG: &str = "omnidrop://drag";
pub const EVENT_JOB: &str = "omnidrop://job";
pub const EVENT_TOOLS: &str = "omnidrop://tools";

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FilesAddedEvent {
    pub files: Vec<FileInfo>,
    /// The import stopped at the file-count limit.
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InspectingEvent {
    pub active: bool,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DragEvent {
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolsPayload {
    /// Tool discovery has finished.
    pub ready: bool,
    pub tools: ToolSet,
    pub capabilities: Vec<ConversionCapability>,
}

impl ToolsPayload {
    pub fn new(ready: bool, tools: ToolSet) -> Self {
        let capabilities = CapabilityRegistry::new(&tools).list().to_vec();
        Self {
            ready,
            tools,
            capabilities,
        }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Bootstrap {
    pub version: String,
    pub platform: String,
    pub settings: Settings,
    pub files: Vec<FileInfo>,
    pub tools: ToolsPayload,
    pub history: Vec<HistoryEntry>,
}

/// The only web pages the app can open, in the user's browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ExternalLink {
    FfmpegDownload,
    Repository,
    Issues,
}

impl ExternalLink {
    pub fn url(self) -> &'static str {
        match self {
            ExternalLink::FfmpegDownload => "https://ffmpeg.org/download.html",
            ExternalLink::Repository => "https://github.com/Ernani1234/Omnidrop",
            ExternalLink::Issues => "https://github.com/Ernani1234/Omnidrop/issues",
        }
    }
}

/// The folder that best represents a job's results: the chosen output
/// folder, or the folder of the first result.
pub fn result_folder(settings: &Settings, outputs: &[&Path]) -> Option<PathBuf> {
    if settings.output_mode == OutputMode::Folder
        && let Some(folder) = &settings.output_folder
    {
        return Some(PathBuf::from(folder));
    }
    outputs.first()?.parent().map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_folder_prefers_chosen_folder() {
        let mut settings = Settings::default();
        let out = Path::new("/docs/report.pdf");
        assert_eq!(
            result_folder(&settings, &[out]),
            Some(PathBuf::from("/docs"))
        );
        assert_eq!(result_folder(&settings, &[]), None);
        settings.output_mode = OutputMode::Folder;
        settings.output_folder = Some("/exports".into());
        assert_eq!(
            result_folder(&settings, &[out]),
            Some(PathBuf::from("/exports"))
        );
    }

    #[test]
    fn tools_payload_lists_capabilities() {
        let payload = ToolsPayload::new(true, ToolSet::none());
        assert_eq!(payload.capabilities.len(), 9);
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["tools"]["ffmpeg"], serde_json::Value::Null);
    }

    #[test]
    fn external_links_are_fixed() {
        let link: ExternalLink = serde_json::from_str("\"ffmpegDownload\"").unwrap();
        assert!(link.url().starts_with("https://ffmpeg.org/"));
        assert!(serde_json::from_str::<ExternalLink>("\"https://evil.example\"").is_err());
    }
}
