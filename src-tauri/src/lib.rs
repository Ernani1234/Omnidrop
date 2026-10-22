//! OmniDrop desktop shell: window, native integration and commands on top
//! of `omnidrop-engine`.

mod commands;
mod state;

use omnidrop_app::payloads::{DragEvent, EVENT_DRAG};
use state::AppState;
use std::path::PathBuf;
use tauri::{DragDropEvent, Emitter, Manager, WindowEvent};

/// Files passed on the command line ("Open with OmniDrop", drag onto the icon).
fn launch_paths() -> Vec<PathBuf> {
    std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .filter(|p| !p.to_string_lossy().starts_with('-') && p.exists())
        .collect()
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: None,
                    }),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                ])
                .level(log::LevelFilter::Info)
                .max_file_size(2_000_000)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            app.manage(AppState::new(
                config_dir.join("settings.json"),
                data_dir.join("history.json"),
            ));
            commands::refresh_tools(app.handle());
            commands::add_paths(app.handle(), launch_paths());
            log::info!("OmniDrop {} started", app.package_info().version);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::DragDrop(drag) = event {
                let app = window.app_handle();
                match drag {
                    DragDropEvent::Enter { .. } | DragDropEvent::Over { .. } => {
                        let _ = app.emit(EVENT_DRAG, DragEvent { active: true });
                    }
                    DragDropEvent::Leave => {
                        let _ = app.emit(EVENT_DRAG, DragEvent { active: false });
                    }
                    DragDropEvent::Drop { paths, .. } => {
                        let _ = app.emit(EVENT_DRAG, DragEvent { active: false });
                        commands::add_paths(app, paths.clone());
                    }
                    _ => {}
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::pick_files,
            commands::pick_folder,
            commands::remove_files,
            commands::clear_files,
            commands::move_file,
            commands::thumbnail,
            commands::preview_plan,
            commands::start_job,
            commands::cancel_job,
            commands::cancel_task,
            commands::open_output,
            commands::reveal_output,
            commands::open_job_folder,
            commands::update_settings,
            commands::pick_output_folder,
            commands::pick_ffmpeg,
            commands::recheck_tools,
            commands::clear_history,
            commands::readd_history,
            commands::open_history_folder,
            commands::open_logs_folder,
            commands::open_external,
        ])
        .build(tauri::generate_context!())
        .expect("error while building OmniDrop");

    app.run(|_handle, _event| {
        // macOS delivers "Open with" files as events instead of arguments.
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Opened { urls } = _event {
            let paths = urls
                .into_iter()
                .filter_map(|url| url.to_file_path().ok())
                .collect();
            commands::add_paths(_handle, paths);
        }
    });
}
