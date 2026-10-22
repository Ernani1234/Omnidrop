//! Recent jobs, kept locally so users can reopen results or repeat work.

use crate::settings::write_json_atomically;
use omnidrop_engine::OperationId;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use ts_rs::TS;

pub const MAX_ENTRIES: usize = 25;
pub const MAX_SOURCES: usize = 500;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryEntry {
    pub id: String,
    pub operation: OperationId,
    /// Unix time in milliseconds.
    #[ts(type = "number")]
    pub finished_at: u64,
    pub succeeded: u32,
    pub failed: u32,
    pub output_count: u32,
    pub output_dir: Option<String>,
    /// Original files, so they can be added again.
    pub sources: Vec<String>,
}

pub fn load(path: &Path) -> Vec<HistoryEntry> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, entries: &[HistoryEntry]) {
    if let Err(err) = write_json_atomically(path, &entries) {
        log::warn!("could not save history: {err}");
    }
}

/// Adds an entry at the front and drops the oldest ones.
pub fn push(entries: &mut Vec<HistoryEntry>, mut entry: HistoryEntry) {
    entry.sources.truncate(MAX_SOURCES);
    entries.insert(0, entry);
    entries.truncate(MAX_ENTRIES);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str) -> HistoryEntry {
        HistoryEntry {
            id: id.into(),
            operation: OperationId::ImageConvert,
            finished_at: 1,
            succeeded: 1,
            failed: 0,
            output_count: 1,
            output_dir: None,
            sources: vec!["a".into(); MAX_SOURCES + 5],
        }
    }

    #[test]
    fn keeps_newest_entries_first() {
        let mut entries = Vec::new();
        for i in 0..(MAX_ENTRIES + 3) {
            push(&mut entries, entry(&i.to_string()));
        }
        assert_eq!(entries.len(), MAX_ENTRIES);
        assert_eq!(entries[0].id, (MAX_ENTRIES + 2).to_string());
        assert_eq!(entries[0].sources.len(), MAX_SOURCES);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        save(&path, &entries);
        assert_eq!(load(&path), entries);
    }
}
