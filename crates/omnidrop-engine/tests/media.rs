//! Audio/video tests run only when FFmpeg is installed (CI installs it on
//! Linux). Without it they only check that the feature is reported as
//! unavailable.

mod common;

use common::*;
use omnidrop_engine::job::{JobEvent, JobHandle};
use omnidrop_engine::operation::{AudioConvertOptions, Preset, VideoConvertOptions};
use omnidrop_engine::plan::TaskNote;
use omnidrop_engine::{
    CapabilityRegistry, FileDetails, FileFormat, Limits, OperationId, OperationRequest, ToolSet,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn tools() -> Option<ToolSet> {
    let tools = ToolSet::discover(None);
    if tools.ffmpeg.is_none() || tools.ffprobe.is_none() {
        eprintln!("FFmpeg not found: skipping media conversion test");
        return None;
    }
    Some(tools)
}

fn generate(tools: &ToolSet, args: &[&str], out: &Path) {
    let ffmpeg = &tools.ffmpeg.as_ref().unwrap().executable;
    let mut full: Vec<std::ffi::OsString> = ["-hide_banner", "-loglevel", "error", "-y"]
        .iter()
        .map(Into::into)
        .collect();
    full.extend(args.iter().map(Into::into));
    full.push(out.as_os_str().to_owned());
    let output =
        omnidrop_engine::tools::run_capture(ffmpeg, full, Duration::from_secs(120)).unwrap();
    assert!(
        output.success,
        "fixture generation failed: {}",
        output.stderr
    );
}

fn tone(tools: &ToolSet, dir: &Path, seconds: u32) -> PathBuf {
    let path = dir.join("tone.wav");
    let src = format!("sine=frequency=440:duration={seconds}");
    generate(tools, &["-f", "lavfi", "-i", &src], &path);
    path
}

fn clip(tools: &ToolSet, dir: &Path) -> PathBuf {
    let path = dir.join("clip.mkv");
    generate(
        tools,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=320x240:rate=15:duration=2",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=330:duration=2",
            "-c:v",
            "mpeg4",
            "-c:a",
            "pcm_s16le",
            "-shortest",
        ],
        &path,
    );
    path
}

#[test]
fn media_requires_ffmpeg() {
    let registry = CapabilityRegistry::new(&ToolSet::none());
    assert!(!registry.get(OperationId::AudioConvert).available);
    assert!(!registry.get(OperationId::VideoConvert).available);
}

#[test]
fn audio_converts_to_every_available_format() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let wav = tone(&tools, dir.path(), 2);
    let files = inspect_all(&[wav], &tools);
    let Some(FileDetails::Media {
        duration_ms,
        has_audio,
        has_video,
        ..
    }) = files[0].details.clone()
    else {
        panic!("probe failed: {:?}", files[0])
    };
    assert!(has_audio && !has_video);
    assert!(duration_ms.unwrap().abs_diff(2000) < 100);

    let registry = CapabilityRegistry::new(&tools);
    for target in registry
        .get(OperationId::AudioConvert)
        .output_formats
        .clone()
    {
        let request = OperationRequest::AudioConvert(AudioConvertOptions {
            target,
            preset: Preset::Balanced,
        });
        let plan = make_plan(
            &files,
            &request,
            &settings_in(&dir.path().join("out")),
            &tools,
        );
        let run = run_plan(&plan, &tools, &Limits::default());
        assert_eq!(run.summary.succeeded, 1, "{target:?}: {:?}", run.events);
        let out = &run.summary.outputs[0].path;
        assert_eq!(out.extension().unwrap(), target.extension());
    }
}

#[test]
fn video_converts_and_extracts_audio() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let source = clip(&tools, dir.path());
    let files = inspect_all(&[source], &tools);
    let registry = CapabilityRegistry::new(&tools);
    for target in registry
        .get(OperationId::VideoConvert)
        .output_formats
        .clone()
    {
        let request = OperationRequest::VideoConvert(VideoConvertOptions {
            target,
            preset: Preset::Smallest,
            max_height: Some(144),
            remove_audio: target == FileFormat::Webm,
        });
        let plan = make_plan(
            &files,
            &request,
            &settings_in(&dir.path().join("video")),
            &tools,
        );
        let run = run_plan(&plan, &tools, &Limits::default());
        assert_eq!(run.summary.succeeded, 1, "{target:?}: {:?}", run.events);
        let out = inspect_all(&[run.summary.outputs[0].path.clone()], &tools).remove(0);
        let Some(FileDetails::Media {
            height, has_audio, ..
        }) = out.details
        else {
            panic!()
        };
        assert_eq!(height, Some(144));
        assert_eq!(has_audio, target != FileFormat::Webm);
    }

    if registry
        .get(OperationId::AudioConvert)
        .output_formats
        .contains(&FileFormat::Flac)
    {
        let request = OperationRequest::AudioConvert(AudioConvertOptions {
            target: FileFormat::Flac,
            preset: Preset::Balanced,
        });
        let plan = make_plan(
            &files,
            &request,
            &settings_in(&dir.path().join("audio")),
            &tools,
        );
        assert_eq!(plan.tasks[0].notes, vec![TaskNote::AudioFromVideo]);
        let run = run_plan(&plan, &tools, &Limits::default());
        assert_eq!(run.summary.succeeded, 1, "{:?}", run.events);
    }
}

#[test]
fn cancelling_kills_ffmpeg_and_leaves_nothing() {
    let Some(tools) = tools() else { return };
    let dir = tempfile::tempdir().unwrap();
    let wav = tone(&tools, dir.path(), 600);
    let files = inspect_all(&[wav], &tools);
    let target = if CapabilityRegistry::new(&tools)
        .get(OperationId::AudioConvert)
        .output_formats
        .contains(&FileFormat::Flac)
    {
        FileFormat::Flac
    } else {
        FileFormat::Wav
    };
    let request = OperationRequest::AudioConvert(AudioConvertOptions {
        target,
        preset: Preset::Smallest,
    });
    let out = dir.path().join("out");
    let plan = make_plan(&files, &request, &settings_in(&out), &tools);
    let handle = JobHandle::new(&plan);
    let run = run_with(
        &plan,
        &handle,
        &tools,
        &Limits::default(),
        |event, handle| {
            if matches!(event, JobEvent::TaskStarted { .. }) {
                handle.cancel_all();
            }
        },
    );
    assert_eq!(run.summary.cancelled, 1, "{:?}", run.events);
    if out.exists() {
        assert!(files_in(&out).is_empty(), "{:?}", files_in(&out));
    }
}
