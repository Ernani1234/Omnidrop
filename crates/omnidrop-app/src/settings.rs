//! User preferences, stored as JSON in the app config folder.

use omnidrop_engine::operation::{CollisionPolicy, OutputLocation, OutputSettings};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum Language {
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "pt-BR")]
    PtBr,
    #[serde(rename = "en")]
    En,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum OutputMode {
    #[default]
    NextToSource,
    Folder,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Settings {
    pub theme: Theme,
    pub language: Language,
    pub output_mode: OutputMode,
    /// Only ever set from the native folder picker.
    pub output_folder: Option<String>,
    pub collision: CollisionPolicy,
    /// Only ever set from the native file picker.
    pub ffmpeg_path: Option<String>,
}

/// Changes the UI may request. Paths are deliberately absent: folders and
/// executables can only be chosen through native dialogs.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct SettingsPatch {
    #[ts(optional)]
    pub theme: Option<Theme>,
    #[ts(optional)]
    pub language: Option<Language>,
    #[ts(optional)]
    pub output_mode: Option<OutputMode>,
    #[ts(optional)]
    pub collision: Option<CollisionPolicy>,
    #[ts(optional)]
    pub clear_ffmpeg_path: Option<bool>,
}

impl Settings {
    pub fn apply(&mut self, patch: SettingsPatch) {
        if let Some(theme) = patch.theme {
            self.theme = theme;
        }
        if let Some(language) = patch.language {
            self.language = language;
        }
        if let Some(mode) = patch.output_mode {
            // A folder must have been picked before it can be used.
            if mode == OutputMode::NextToSource || self.output_folder.is_some() {
                self.output_mode = mode;
            }
        }
        if let Some(collision) = patch.collision {
            self.collision = collision;
        }
        if patch.clear_ffmpeg_path == Some(true) {
            self.ffmpeg_path = None;
        }
    }

    pub fn output_settings(&self) -> OutputSettings {
        let location = match (&self.output_mode, &self.output_folder) {
            (OutputMode::Folder, Some(folder)) => OutputLocation::Folder {
                path: folder.clone(),
            },
            _ => OutputLocation::NextToSource,
        };
        OutputSettings {
            location,
            collision: self.collision,
        }
    }

    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) {
        if let Err(err) = write_json_atomically(path, self) {
            log::warn!("could not save settings: {err}");
        }
    }
}

/// Writes JSON to a temporary file and renames it over the target, so a
/// crash never leaves a half-written file.
pub fn write_json_atomically<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let dir = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    fs::create_dir_all(&dir)?;
    let tmp = dir.join(format!(
        ".{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_mode_requires_a_picked_folder() {
        let mut settings = Settings::default();
        settings.apply(SettingsPatch {
            output_mode: Some(OutputMode::Folder),
            ..Default::default()
        });
        assert_eq!(settings.output_mode, OutputMode::NextToSource);
        settings.output_folder = Some("/out".into());
        settings.apply(SettingsPatch {
            output_mode: Some(OutputMode::Folder),
            ..Default::default()
        });
        assert_eq!(
            settings.output_settings().location,
            OutputLocation::Folder {
                path: "/out".into()
            }
        );
    }

    #[test]
    fn round_trips_and_tolerates_bad_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let settings = Settings {
            theme: Theme::Dark,
            language: Language::En,
            collision: CollisionPolicy::Skip,
            ..Default::default()
        };
        settings.save(&path);
        assert_eq!(Settings::load(&path), settings);
        fs::write(&path, "{ not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
        let json = serde_json::to_string(&Language::PtBr).unwrap();
        assert_eq!(json, "\"pt-BR\"");
    }
}
