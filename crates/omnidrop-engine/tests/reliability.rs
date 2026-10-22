//! Failure handling: corrupt input, isolated failures, collisions,
//! cancellation and large batches.

mod common;

use common::*;
use omnidrop_engine::job::{JobEvent, JobHandle, TaskOutcome};
use omnidrop_engine::operation::{CollisionPolicy, ImageConvertOptions, Preset};
use omnidrop_engine::{ErrorCode, FileFormat, Limits, OperationRequest, ToolSet};
use std::path::PathBuf;

fn to_jpeg() -> OperationRequest {
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
fn corrupt_file_fails_alone_and_batch_continues() {
    let dir = tempfile::tempdir().unwrap();
    let good_a = dir.path().join("a.png");
    let good_b = dir.path().join("c.png");
    write_png(&good_a, 16, 16, false);
    write_png(&good_b, 16, 16, false);
    // Valid header (inspection succeeds) but the pixel data is cut short,
    // so the failure only shows up while converting.
    let broken = dir.path().join("b.png");
    let noise = image::RgbImage::from_fn(128, 128, |x, y| {
        image::Rgb([(x * y) as u8, (x ^ y) as u8, (x + y) as u8])
    });
    noise.save(&broken).unwrap();
    let mut bytes = std::fs::read(&broken).unwrap();
    bytes.truncate(bytes.len() * 2 / 3);
    std::fs::write(&broken, bytes).unwrap();

    let run = convert(
        &[good_a, broken.clone(), good_b],
        to_jpeg(),
        &settings(CollisionPolicy::Rename),
    );
    assert_eq!(run.summary.succeeded, 2);
    assert_eq!(run.summary.failed, 1);
    let error = run.events.iter().find_map(|e| match e {
        JobEvent::TaskFinished {
            outcome: TaskOutcome::Failed { error },
            ..
        } => Some(error.code),
        _ => None,
    });
    assert_eq!(error, Some(ErrorCode::Unreadable));
    assert!(no_temp_files(dir.path()));
    assert!(!dir.path().join("b.jpg").exists());
}

#[test]
fn unknown_and_unreadable_inputs_are_explained_in_the_plan() {
    let dir = tempfile::tempdir().unwrap();
    let text = dir.path().join("notes.txt");
    std::fs::write(&text, "hello").unwrap();
    let garbage = dir.path().join("photo.png");
    std::fs::write(
        &garbage,
        [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3],
    )
    .unwrap();
    let tools = ToolSet::none();
    let files = inspect_all(&[text, garbage], &tools);
    let plan = make_plan(
        &files,
        &to_jpeg(),
        &settings(CollisionPolicy::Rename),
        &tools,
    );
    assert!(plan.tasks.is_empty());
    let codes: Vec<_> = plan.skipped.iter().map(|s| s.error.code).collect();
    assert_eq!(
        codes,
        vec![ErrorCode::UnsupportedFormat, ErrorCode::Unreadable]
    );
}

#[test]
fn collisions_follow_policy_and_never_touch_existing_files() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("pic.png");
    write_png(&src, 8, 8, false);
    let existing = dir.path().join("pic.jpg");
    std::fs::write(&existing, b"precious").unwrap();

    let run = convert(
        std::slice::from_ref(&src),
        to_jpeg(),
        &settings(CollisionPolicy::Rename),
    );
    assert_eq!(run.summary.outputs[0].path, dir.path().join("pic (1).jpg"));
    assert_eq!(std::fs::read(&existing).unwrap(), b"precious");

    let run = convert(
        std::slice::from_ref(&src),
        to_jpeg(),
        &settings(CollisionPolicy::Skip),
    );
    assert_eq!(run.summary.skipped, 1);
    let skipped = run.events.iter().find_map(|e| match e {
        JobEvent::TaskFinished {
            outcome: TaskOutcome::Skipped { error },
            ..
        } => Some(error.code),
        _ => None,
    });
    assert_eq!(skipped, Some(ErrorCode::OutputExists));
    assert_eq!(std::fs::read(&existing).unwrap(), b"precious");

    let run = convert(&[src], to_jpeg(), &settings(CollisionPolicy::Overwrite));
    assert_eq!(run.summary.outputs[0].path, existing);
    assert_ne!(std::fs::read(&existing).unwrap(), b"precious");
    assert!(no_temp_files(dir.path()));
}

#[test]
fn same_name_inputs_in_one_batch_do_not_overwrite_each_other() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("photo.png");
    let bmp = dir.path().join("photo.bmp");
    write_png(&png, 8, 8, false);
    write_image(&bmp, image::ImageFormat::Bmp, 8, 8);
    let run = convert(
        &[png, bmp],
        to_jpeg(),
        &settings(CollisionPolicy::Overwrite),
    );
    assert_eq!(run.summary.succeeded, 2);
    let mut names = files_in(dir.path());
    names.retain(|n| n.ends_with(".jpg"));
    assert_eq!(names, vec!["photo (1).jpg", "photo.jpg"]);
}

#[test]
fn cancelling_before_start_processes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let paths: Vec<PathBuf> = (0..5)
        .map(|i| {
            let p = dir.path().join(format!("{i}.png"));
            write_png(&p, 8, 8, false);
            p
        })
        .collect();
    let tools = ToolSet::none();
    let files = inspect_all(&paths, &tools);
    let plan = make_plan(
        &files,
        &to_jpeg(),
        &settings(CollisionPolicy::Rename),
        &tools,
    );
    let handle = JobHandle::new(&plan);
    handle.cancel_all();
    let run = run_with(&plan, &handle, &tools, &Limits::default(), |_, _| {});
    assert_eq!(run.summary.cancelled, 5);
    assert_eq!(files_in(dir.path()).len(), 5, "no outputs should exist");
}

#[test]
fn cancelling_during_a_batch_stops_remaining_items_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let paths: Vec<PathBuf> = (0..40)
        .map(|i| {
            let p = dir.path().join(format!("{i:02}.png"));
            write_png(&p, 96, 96, false);
            p
        })
        .collect();
    let tools = ToolSet::none();
    let files = inspect_all(&paths, &tools);
    let plan = make_plan(
        &files,
        &to_jpeg(),
        &settings(CollisionPolicy::Rename),
        &tools,
    );
    let handle = JobHandle::new(&plan);
    let run = run_with(
        &plan,
        &handle,
        &tools,
        &Limits::default(),
        |event, handle| {
            if matches!(event, JobEvent::TaskFinished { .. }) {
                handle.cancel_all();
            }
        },
    );
    let s = &run.summary;
    assert!(s.succeeded >= 1);
    assert!(s.cancelled >= 1, "{s:?}");
    assert_eq!(s.succeeded + s.cancelled + s.failed, 40);
    assert_eq!(s.failed, 0);
    // Exactly the reported outputs exist; nothing half-written remains.
    let jpgs = files_in(dir.path())
        .into_iter()
        .filter(|n| n.ends_with(".jpg"))
        .count();
    assert_eq!(jpgs as u32, s.succeeded);
    assert!(no_temp_files(dir.path()));
}

#[test]
fn cancelling_one_task_keeps_the_others() {
    let dir = tempfile::tempdir().unwrap();
    let paths: Vec<PathBuf> = (0..3)
        .map(|i| {
            let p = dir.path().join(format!("{i}.png"));
            write_png(&p, 8, 8, false);
            p
        })
        .collect();
    let tools = ToolSet::none();
    let files = inspect_all(&paths, &tools);
    let plan = make_plan(
        &files,
        &to_jpeg(),
        &settings(CollisionPolicy::Rename),
        &tools,
    );
    let handle = JobHandle::new(&plan);
    assert!(handle.cancel_task(&plan.tasks[1].id));
    assert!(!handle.cancel_task("unknown"));
    let run = run_with(&plan, &handle, &tools, &Limits::default(), |_, _| {});
    assert_eq!(run.summary.succeeded, 2);
    assert_eq!(run.summary.cancelled, 1);
    assert!(!dir.path().join("1.jpg").exists());
}

#[test]
fn large_batch_completes() {
    let dir = tempfile::tempdir().unwrap();
    let paths: Vec<PathBuf> = (0..200)
        .map(|i| {
            let p = dir.path().join(format!("img-{i:03}.png"));
            write_png(&p, 12, 12, i % 3 == 0);
            p
        })
        .collect();
    let out = tempfile::tempdir().unwrap();
    let run = convert(&paths, to_jpeg(), &settings_in(out.path()));
    assert_eq!(run.summary.succeeded, 200);
    assert_eq!(files_in(out.path()).len(), 200);
}

#[cfg(unix)]
#[test]
fn read_only_destination_reports_actionable_error() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.png");
    write_png(&src, 8, 8, false);
    let locked = dir.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    // Root ignores permissions; nothing meaningful to test then.
    if std::fs::write(locked.join("probe"), b"x").is_ok() {
        return;
    }
    let run = convert(&[src], to_jpeg(), &settings_in(&locked));
    let error = run.events.iter().find_map(|e| match e {
        JobEvent::TaskFinished {
            outcome: TaskOutcome::Failed { error },
            ..
        } => Some(error.code),
        _ => None,
    });
    assert_eq!(error, Some(ErrorCode::DestinationNotWritable));
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn folder_import_mirrors_structure_in_output_folder() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Trip");
    std::fs::create_dir_all(root.join("Day 1")).unwrap();
    write_png(&root.join("cover.png"), 8, 8, false);
    write_png(&root.join("Day 1").join("beach.png"), 8, 8, false);
    let tools = ToolSet::none();
    let expanded = omnidrop_engine::inspector::expand_inputs(&[root], &Limits::default());
    let files: Vec<_> = expanded
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            omnidrop_engine::inspector::inspect(e, i.to_string(), &tools, &Limits::default())
        })
        .collect();
    let out = tempfile::tempdir().unwrap();
    let plan = make_plan(&files, &to_jpeg(), &settings_in(out.path()), &tools);
    let run = run_plan(&plan, &tools, &Limits::default());
    assert_eq!(run.summary.succeeded, 2);
    assert!(out.path().join("Trip").join("cover.jpg").is_file());
    assert!(
        out.path()
            .join("Trip")
            .join("Day 1")
            .join("beach.jpg")
            .is_file()
    );
}
