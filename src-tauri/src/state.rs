//! Process-wide state shared by commands and background jobs.

use omnidrop_app::history::{self, HistoryEntry};
use omnidrop_app::settings::Settings;
use omnidrop_engine::{CapabilityRegistry, FileInfo, JobHandle, Limits, ToolSet};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};

pub struct AppState {
    pub limits: Limits,
    /// The queue, in display order. Paths here only come from native
    /// drag-and-drop, native dialogs or launch arguments.
    pub files: Mutex<Vec<FileInfo>>,
    pub tools: RwLock<ToolSet>,
    pub tools_ready: AtomicBool,
    pub settings: Mutex<Settings>,
    pub jobs: Mutex<HashMap<String, Arc<JobHandle>>>,
    /// Results that may be opened or revealed, by output id.
    pub outputs: Mutex<HashMap<String, PathBuf>>,
    /// Folder to open for each finished job.
    pub job_dirs: Mutex<HashMap<String, PathBuf>>,
    pub thumbnails: Mutex<HashMap<String, Option<String>>>,
    pub history: Mutex<Vec<HistoryEntry>>,
    pub settings_path: PathBuf,
    pub history_path: PathBuf,
}

impl AppState {
    pub fn new(settings_path: PathBuf, history_path: PathBuf) -> Self {
        let settings = Settings::load(&settings_path);
        let history = history::load(&history_path);
        Self {
            limits: Limits::default(),
            files: Mutex::new(Vec::new()),
            tools: RwLock::new(ToolSet::none()),
            tools_ready: AtomicBool::new(false),
            settings: Mutex::new(settings),
            jobs: Mutex::new(HashMap::new()),
            outputs: Mutex::new(HashMap::new()),
            job_dirs: Mutex::new(HashMap::new()),
            thumbnails: Mutex::new(HashMap::new()),
            history: Mutex::new(history),
            settings_path,
            history_path,
        }
    }

    pub fn registry(&self) -> CapabilityRegistry {
        CapabilityRegistry::new(&self.tools.read().expect("tools lock"))
    }

    pub fn settings(&self) -> Settings {
        self.settings.lock().expect("settings lock").clone()
    }
}
