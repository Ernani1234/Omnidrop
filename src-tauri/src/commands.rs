//! Commands the UI can call. The UI refers to files and results by id; it
//! never sends file-system paths that the backend would trust.

use crate::state::AppState;
use omnidrop_app::history::{self, HistoryEntry};
use omnidrop_app::payloads::{
    Bootstrap, EVENT_FILES, EVENT_INSPECTING, EVENT_JOB, EVENT_TOOLS, ExternalLink,
    FilesAddedEvent, InspectingEvent, ToolsPayload, result_folder,
};
use omnidrop_app::settings::{OutputMode, Settings, SettingsPatch};
use omnidrop_app::thumbnails;
use omnidrop_engine::inspector::{self, InputEntry};
use omnidrop_engine::job::{JobEvent, JobHandle, TaskOutcome};
use omnidrop_engine::{
    EngineError, ErrorCode, FileInfo, FormatFamily, OperationRequest, Plan, ToolSet, run_job,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

type CommandResult<T> = Result<T, EngineError>;

fn not_found(what: &str) -> EngineError {
    EngineError::new(ErrorCode::NotFound, format!("{what} not found"))
}

pub fn tools_payload(state: &AppState) -> ToolsPayload {
    ToolsPayload::new(
        state.tools_ready.load(Ordering::SeqCst),
        state.tools.read().expect("tools lock").clone(),
    )
}

/// Looks for FFmpeg in the background and tells the UI what is available.
pub fn refresh_tools(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let custom = state.settings().ffmpeg_path.map(PathBuf::from);
        let tools = ToolSet::discover(custom.as_deref());
        log::info!(
            "FFmpeg {}",
            tools
                .ffmpeg
                .as_ref()
                .map(|t| t.version.as_str())
                .unwrap_or("not found")
        );
        *state.tools.write().expect("tools lock") = tools;
        state.tools_ready.store(true, Ordering::SeqCst);
        let _ = app.emit(EVENT_TOOLS, tools_payload(&state));
    });
}

/// Inspects paths that came from a native source and appends them to the queue.
pub fn add_paths(app: &AppHandle, paths: Vec<PathBuf>) {
    if paths.is_empty() {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let expanded = inspector::expand_inputs(&paths, &state.limits);
        let _ = app.emit(
            EVENT_INSPECTING,
            InspectingEvent {
                active: true,
                count: expanded.entries.len() as u32,
            },
        );
        let tools = state.tools.read().expect("tools lock").clone();
        let infos = inspect_parallel(&expanded.entries, &tools, &state.limits);
        let added = {
            let mut files = state.files.lock().expect("files lock");
            let mut added = Vec::new();
            for info in infos {
                // Adding the same file twice is almost always a mistake.
                if files.iter().any(|f| f.path == info.path) {
                    continue;
                }
                files.push(info.clone());
                added.push(info);
            }
            added
        };
        let _ = app.emit(
            EVENT_INSPECTING,
            InspectingEvent {
                active: false,
                count: 0,
            },
        );
        let _ = app.emit(
            EVENT_FILES,
            FilesAddedEvent {
                files: added,
                truncated: expanded.truncated,
            },
        );
    });
}

fn inspect_parallel(
    entries: &[InputEntry],
    tools: &ToolSet,
    limits: &omnidrop_engine::Limits,
) -> Vec<FileInfo> {
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2)
        .clamp(1, 6);
    let chunk = entries.len().div_ceil(workers).max(1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = entries
            .chunks(chunk)
            .map(|part| {
                scope.spawn(move || {
                    part.iter()
                        .map(|entry| {
                            inspector::inspect(
                                entry,
                                uuid::Uuid::new_v4().to_string(),
                                tools,
                                limits,
                            )
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_default())
            .collect()
    })
}

#[tauri::command(async)]
pub fn bootstrap(app: AppHandle, state: State<'_, AppState>) -> Bootstrap {
    Bootstrap {
        version: app.package_info().version.to_string(),
        platform: std::env::consts::OS.to_string(),
        settings: state.settings(),
        files: state.files.lock().expect("files lock").clone(),
        tools: tools_payload(&state),
        history: state.history.lock().expect("history lock").clone(),
    }
}

fn family_filter(family: FormatFamily) -> (&'static str, Vec<&'static str>) {
    use omnidrop_engine::capability::*;
    let formats: Vec<omnidrop_engine::FileFormat> = match family {
        FormatFamily::Image => IMAGE_INPUTS.to_vec(),
        FormatFamily::Pdf => vec![omnidrop_engine::FileFormat::Pdf],
        FormatFamily::Audio => AUDIO_INPUTS.to_vec(),
        FormatFamily::Video => VIDEO_INPUTS.to_vec(),
        FormatFamily::Archive => ARCHIVE_FORMATS.to_vec(),
        FormatFamily::Document => vec![],
    };
    let mut extensions: Vec<&'static str> = formats
        .iter()
        .flat_map(|f| f.extensions().iter().copied())
        .map(|e| e.trim_start_matches("tar."))
        .collect();
    extensions.sort_unstable();
    extensions.dedup();
    let name = match family {
        FormatFamily::Image => "Images",
        FormatFamily::Pdf => "PDF",
        FormatFamily::Audio => "Audio",
        FormatFamily::Video => "Video",
        FormatFamily::Archive => "Archives",
        FormatFamily::Document => "Documents",
    };
    (name, extensions)
}

#[tauri::command(async)]
pub fn pick_files(app: AppHandle, family: Option<FormatFamily>) {
    let mut dialog = app.dialog().file();
    if let Some(family) = family {
        let (name, extensions) = family_filter(family);
        if !extensions.is_empty() {
            dialog = dialog.add_filter(name, &extensions);
        }
    }
    if let Some(picked) = dialog.blocking_pick_files() {
        let paths = picked
            .into_iter()
            .filter_map(|p| p.into_path().ok())
            .collect();
        add_paths(&app, paths);
    }
}

#[tauri::command(async)]
pub fn pick_folder(app: AppHandle) {
    if let Some(folder) = app.dialog().file().blocking_pick_folder()
        && let Ok(path) = folder.into_path()
    {
        add_paths(&app, vec![path]);
    }
}

#[tauri::command(async)]
pub fn remove_files(state: State<'_, AppState>, ids: Vec<String>) {
    state
        .files
        .lock()
        .expect("files lock")
        .retain(|f| !ids.contains(&f.id));
    let mut thumbs = state.thumbnails.lock().expect("thumbnail lock");
    for id in &ids {
        thumbs.remove(id);
    }
}

#[tauri::command(async)]
pub fn clear_files(state: State<'_, AppState>) {
    state.files.lock().expect("files lock").clear();
    state.thumbnails.lock().expect("thumbnail lock").clear();
}

/// Moves a file within the queue (merge order follows the queue).
#[tauri::command(async)]
pub fn move_file(state: State<'_, AppState>, id: String, to_index: usize) -> CommandResult<()> {
    let mut files = state.files.lock().expect("files lock");
    let from = files
        .iter()
        .position(|f| f.id == id)
        .ok_or_else(|| not_found("file"))?;
    let file = files.remove(from);
    let to = to_index.min(files.len());
    files.insert(to, file);
    Ok(())
}

#[tauri::command(async)]
pub fn thumbnail(state: State<'_, AppState>, id: String) -> Option<String> {
    if let Some(cached) = state.thumbnails.lock().expect("thumbnail lock").get(&id) {
        return cached.clone();
    }
    let file = state
        .files
        .lock()
        .expect("files lock")
        .iter()
        .find(|f| f.id == id)
        .cloned()?;
    let thumb = thumbnails::make(&file, &state.limits);
    state
        .thumbnails
        .lock()
        .expect("thumbnail lock")
        .insert(id, thumb.clone());
    thumb
}

fn selected_files(state: &AppState, file_ids: Option<&[String]>) -> Vec<FileInfo> {
    let files = state.files.lock().expect("files lock");
    match file_ids {
        Some(ids) => files
            .iter()
            .filter(|f| ids.contains(&f.id))
            .cloned()
            .collect(),
        None => files.clone(),
    }
}

fn build_plan(
    state: &AppState,
    request: &OperationRequest,
    file_ids: Option<&[String]>,
) -> CommandResult<Plan> {
    let files = selected_files(state, file_ids);
    let settings = state.settings().output_settings();
    omnidrop_engine::plan(&files, request, &settings, &state.registry())
}

#[tauri::command(async)]
pub fn preview_plan(
    state: State<'_, AppState>,
    request: OperationRequest,
    file_ids: Option<Vec<String>>,
) -> CommandResult<Plan> {
    build_plan(&state, &request, file_ids.as_deref())
}

#[tauri::command(async)]
pub fn start_job(
    app: AppHandle,
    state: State<'_, AppState>,
    request: OperationRequest,
    file_ids: Option<Vec<String>>,
) -> CommandResult<Plan> {
    let plan = build_plan(&state, &request, file_ids.as_deref())?;
    if plan.tasks.is_empty() {
        return Err(EngineError::new(
            ErrorCode::NotEnoughFiles,
            "no compatible files",
        ));
    }
    let handle = Arc::new(JobHandle::new(&plan));
    state
        .jobs
        .lock()
        .expect("jobs lock")
        .insert(plan.job_id.clone(), handle.clone());
    let job_plan = plan.clone();
    std::thread::spawn(move || run_in_background(app, job_plan, handle));
    Ok(plan)
}

fn run_in_background(app: AppHandle, plan: Plan, handle: Arc<JobHandle>) {
    let state = app.state::<AppState>();
    let tools = state.tools.read().expect("tools lock").clone();
    let sink = |event: JobEvent| {
        if let JobEvent::TaskFinished {
            outcome: TaskOutcome::Succeeded { outputs, .. },
            ..
        } = &event
        {
            let mut known = state.outputs.lock().expect("outputs lock");
            for output in outputs {
                known.insert(output.id.clone(), output.path.clone());
            }
        }
        let _ = app.emit(EVENT_JOB, &event);
    };
    let summary = run_job(&plan, &handle, &tools, &state.limits, &sink);

    let paths: Vec<&Path> = summary.outputs.iter().map(|o| o.path.as_path()).collect();
    let folder = result_folder(&state.settings(), &paths);
    if let Some(folder) = &folder {
        state
            .job_dirs
            .lock()
            .expect("job dirs lock")
            .insert(plan.job_id.clone(), folder.clone());
    }
    let entry = HistoryEntry {
        id: plan.job_id.clone(),
        operation: plan.operation,
        finished_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
        succeeded: summary.succeeded,
        failed: summary.failed,
        output_count: summary.outputs.len() as u32,
        output_dir: folder.map(|f| f.to_string_lossy().into_owned()),
        sources: plan
            .tasks
            .iter()
            .flat_map(|t| {
                t.inputs
                    .iter()
                    .map(|i| i.path.to_string_lossy().into_owned())
            })
            .collect(),
    };
    if summary.succeeded > 0 {
        let mut entries = state.history.lock().expect("history lock");
        history::push(&mut entries, entry);
        history::save(&state.history_path, &entries);
    }
    state.jobs.lock().expect("jobs lock").remove(&plan.job_id);
}

#[tauri::command(async)]
pub fn cancel_job(state: State<'_, AppState>, job_id: String) -> CommandResult<()> {
    let jobs = state.jobs.lock().expect("jobs lock");
    let handle = jobs.get(&job_id).ok_or_else(|| not_found("job"))?;
    handle.cancel_all();
    Ok(())
}

#[tauri::command(async)]
pub fn cancel_task(
    state: State<'_, AppState>,
    job_id: String,
    task_id: String,
) -> CommandResult<()> {
    let jobs = state.jobs.lock().expect("jobs lock");
    let handle = jobs.get(&job_id).ok_or_else(|| not_found("job"))?;
    if handle.cancel_task(&task_id) {
        Ok(())
    } else {
        Err(not_found("task"))
    }
}

fn output_path(state: &AppState, output_id: &str) -> CommandResult<PathBuf> {
    state
        .outputs
        .lock()
        .expect("outputs lock")
        .get(output_id)
        .cloned()
        .ok_or_else(|| not_found("result"))
}

fn opener_error(err: impl std::fmt::Display) -> EngineError {
    EngineError::new(ErrorCode::Internal, format!("could not open: {err}"))
}

#[tauri::command(async)]
pub fn open_output(
    app: AppHandle,
    state: State<'_, AppState>,
    output_id: String,
) -> CommandResult<()> {
    let path = output_path(&state, &output_id)?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(opener_error)
}

#[tauri::command(async)]
pub fn reveal_output(
    app: AppHandle,
    state: State<'_, AppState>,
    output_id: String,
) -> CommandResult<()> {
    let path = output_path(&state, &output_id)?;
    app.opener().reveal_item_in_dir(path).map_err(opener_error)
}

#[tauri::command(async)]
pub fn open_job_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    job_id: String,
) -> CommandResult<()> {
    let folder = state
        .job_dirs
        .lock()
        .expect("job dirs lock")
        .get(&job_id)
        .cloned()
        .ok_or_else(|| not_found("folder"))?;
    app.opener()
        .open_path(folder.to_string_lossy(), None::<&str>)
        .map_err(opener_error)
}

#[tauri::command(async)]
pub fn update_settings(state: State<'_, AppState>, patch: SettingsPatch) -> Settings {
    let mut settings = state.settings.lock().expect("settings lock");
    settings.apply(patch);
    settings.save(&state.settings_path);
    settings.clone()
}

#[tauri::command(async)]
pub fn pick_output_folder(app: AppHandle, state: State<'_, AppState>) -> Settings {
    if let Some(folder) = app.dialog().file().blocking_pick_folder()
        && let Ok(path) = folder.into_path()
    {
        let mut settings = state.settings.lock().expect("settings lock");
        settings.output_folder = Some(path.to_string_lossy().into_owned());
        settings.output_mode = OutputMode::Folder;
        settings.save(&state.settings_path);
    }
    state.settings()
}

#[tauri::command(async)]
pub fn pick_ffmpeg(app: AppHandle, state: State<'_, AppState>) -> Settings {
    if let Some(file) = app.dialog().file().blocking_pick_file()
        && let Ok(path) = file.into_path()
    {
        {
            let mut settings = state.settings.lock().expect("settings lock");
            settings.ffmpeg_path = Some(path.to_string_lossy().into_owned());
            settings.save(&state.settings_path);
        }
        state.tools_ready.store(false, Ordering::SeqCst);
        refresh_tools(&app);
    }
    state.settings()
}

#[tauri::command(async)]
pub fn recheck_tools(app: AppHandle, state: State<'_, AppState>) {
    state.tools_ready.store(false, Ordering::SeqCst);
    let _ = app.emit(EVENT_TOOLS, tools_payload(&state));
    refresh_tools(&app);
}

#[tauri::command(async)]
pub fn clear_history(state: State<'_, AppState>) {
    let mut entries = state.history.lock().expect("history lock");
    entries.clear();
    history::save(&state.history_path, &entries);
}

fn history_entry(state: &AppState, entry_id: &str) -> CommandResult<HistoryEntry> {
    state
        .history
        .lock()
        .expect("history lock")
        .iter()
        .find(|e| e.id == entry_id)
        .cloned()
        .ok_or_else(|| not_found("history entry"))
}

/// Adds the original files of a past job back to the queue.
#[tauri::command(async)]
pub fn readd_history(
    app: AppHandle,
    state: State<'_, AppState>,
    entry_id: String,
) -> CommandResult<u32> {
    let entry = history_entry(&state, &entry_id)?;
    let paths: Vec<PathBuf> = entry
        .sources
        .iter()
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .collect();
    let count = paths.len() as u32;
    add_paths(&app, paths);
    Ok(count)
}

#[tauri::command(async)]
pub fn open_history_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    entry_id: String,
) -> CommandResult<()> {
    let entry = history_entry(&state, &entry_id)?;
    let folder = entry.output_dir.ok_or_else(|| not_found("folder"))?;
    if !Path::new(&folder).is_dir() {
        return Err(not_found("folder"));
    }
    app.opener()
        .open_path(folder, None::<&str>)
        .map_err(opener_error)
}

#[tauri::command(async)]
pub fn open_logs_folder(app: AppHandle) -> CommandResult<()> {
    let dir = app.path().app_log_dir().map_err(opener_error)?;
    std::fs::create_dir_all(&dir).map_err(|e| EngineError::write(&e))?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(opener_error)
}

#[tauri::command(async)]
pub fn open_external(app: AppHandle, link: ExternalLink) -> CommandResult<()> {
    app.opener()
        .open_url(link.url(), None::<&str>)
        .map_err(opener_error)
}
