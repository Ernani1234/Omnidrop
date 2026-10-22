mod common;

use common::*;
use omnidrop_engine::job::TaskOutcome;
use omnidrop_engine::operation::{CollisionPolicy, ImageConvertOptions, Preset, ResizeSpec};
use omnidrop_engine::plan::TaskNote;
use omnidrop_engine::{FileFormat, OperationRequest};
use std::path::PathBuf;

fn request(target: FileFormat) -> ImageConvertOptions {
    ImageConvertOptions {
        target,
        preset: Preset::Balanced,
        quality: None,
        lossless: false,
        resize: None,
        background: None,
    }
}

fn outputs(run: &Run) -> Vec<PathBuf> {
    run.summary.outputs.iter().map(|o| o.path.clone()).collect()
}

#[test]
fn every_declared_input_converts_to_every_output() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = [
        ("in.png", image::ImageFormat::Png),
        ("in.jpg", image::ImageFormat::Jpeg),
        ("in.webp", image::ImageFormat::WebP),
        ("in.tiff", image::ImageFormat::Tiff),
        ("in.bmp", image::ImageFormat::Bmp),
        ("in.gif", image::ImageFormat::Gif),
        ("in.ico", image::ImageFormat::Ico),
    ];
    let mut paths = vec![];
    for (name, format) in inputs {
        let path = dir.path().join(name);
        write_image(&path, format, 24, 16);
        paths.push(path);
    }
    for target in [
        FileFormat::Png,
        FileFormat::Jpeg,
        FileFormat::Webp,
        FileFormat::Avif,
        FileFormat::Tiff,
        FileFormat::Bmp,
    ] {
        let out = tempfile::tempdir().unwrap();
        let run = convert(
            &paths,
            OperationRequest::ImageConvert(request(target)),
            &settings_in(out.path()),
        );
        assert_eq!(
            run.summary.succeeded,
            inputs.len() as u32,
            "{target:?}: {:?}",
            run.events
        );
        assert_eq!(run.summary.failed, 0);
        for path in outputs(&run) {
            assert_eq!(path.extension().unwrap(), target.extension());
            let header = std::fs::read(&path).unwrap();
            assert_eq!(
                omnidrop_engine::format::sniff(&header),
                Some(target),
                "{path:?}"
            );
        }
        assert!(no_temp_files(out.path()));
    }
}

#[test]
fn png_to_webp_batch_keeps_originals() {
    let dir = tempfile::tempdir().unwrap();
    let paths: Vec<PathBuf> = (0..30)
        .map(|i| {
            let p = dir.path().join(format!("photo-{i:02}.png"));
            write_png(&p, 64, 48, i % 2 == 0);
            p
        })
        .collect();
    let before: Vec<Vec<u8>> = paths.iter().map(|p| std::fs::read(p).unwrap()).collect();
    let run = convert(
        &paths,
        OperationRequest::ImageConvert(request(FileFormat::Webp)),
        &settings(CollisionPolicy::Rename),
    );
    assert_eq!(run.summary.succeeded, 30);
    assert_eq!(run.summary.outputs.len(), 30);
    for (path, original) in paths.iter().zip(before) {
        assert_eq!(std::fs::read(path).unwrap(), original, "original changed");
        let webp = path.with_extension("webp");
        let decoded = image::open(&webp).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (64, 48));
    }
    // Every task reported progress and a final event.
    let finished = run
        .events
        .iter()
        .filter(|e| matches!(e, omnidrop_engine::JobEvent::TaskFinished { .. }))
        .count();
    assert_eq!(finished, 30);
    assert!(matches!(
        run.events.last(),
        Some(omnidrop_engine::JobEvent::JobFinished { .. })
    ));
}

#[test]
fn jpeg_flattens_transparency_on_chosen_background() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("logo.png");
    write_png(&path, 20, 10, true);
    let mut options = request(FileFormat::Jpeg);
    options.background = Some("#ff0000".into());
    options.quality = Some(100);
    let run = convert(
        &[path],
        OperationRequest::ImageConvert(options),
        &settings(CollisionPolicy::Rename),
    );
    let TaskOutcome::Succeeded { notes, outputs } = task_outcomes(&run).remove(0) else {
        panic!("conversion failed: {:?}", run.events)
    };
    assert!(notes.contains(&TaskNote::TransparencyFlattened));
    let jpeg = image::open(&outputs[0].path).unwrap().to_rgb8();
    let left = jpeg.get_pixel(1, 5);
    assert!(
        left[0] > 200 && left[1] < 60,
        "transparent area should be red, got {left:?}"
    );
}

#[test]
fn resize_fits_and_keeps_aspect_ratio() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wide.png");
    write_png(&path, 400, 200, false);
    let mut options = request(FileFormat::Png);
    options.resize = Some(ResizeSpec::Fit {
        max_width: Some(100),
        max_height: Some(100),
    });
    let run = convert(
        std::slice::from_ref(&path),
        OperationRequest::ImageConvert(options),
        &settings(CollisionPolicy::Rename),
    );
    let out = &run.summary.outputs[0];
    // Same format: the original is protected and a numbered name is used.
    assert_eq!(out.path, dir.path().join("wide (1).png"));
    assert!(out.renamed);
    let img = image::open(&out.path).unwrap();
    assert_eq!((img.width(), img.height()), (100, 50));
}

#[test]
fn webp_lossless_and_quality_override() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.png");
    write_png(&path, 32, 32, false);
    let mut options = request(FileFormat::Webp);
    options.lossless = true;
    let run = convert(
        std::slice::from_ref(&path),
        OperationRequest::ImageConvert(options),
        &settings(CollisionPolicy::Rename),
    );
    let webp = image::open(&run.summary.outputs[0].path).unwrap().to_rgb8();
    let png = image::open(&path).unwrap().to_rgb8();
    assert_eq!(webp, png, "lossless WebP must keep every pixel");

    let mut small = request(FileFormat::Jpeg);
    small.quality = Some(10);
    let mut big = request(FileFormat::Jpeg);
    big.quality = Some(95);
    let photo = dir.path().join("photo.png");
    write_png(&photo, 300, 300, false);
    let low = convert(
        std::slice::from_ref(&photo),
        OperationRequest::ImageConvert(small),
        &settings_in(&dir.path().join("low")),
    );
    let high = convert(
        &[photo],
        OperationRequest::ImageConvert(big),
        &settings_in(&dir.path().join("high")),
    );
    assert!(low.summary.output_bytes < high.summary.output_bytes);
}

#[test]
fn misleading_extension_is_converted_by_content() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("really-a-png.jpg");
    write_png(&path, 10, 10, false);
    let run = convert(
        &[path],
        OperationRequest::ImageConvert(request(FileFormat::Webp)),
        &settings(CollisionPolicy::Rename),
    );
    assert_eq!(run.summary.succeeded, 1);
}

#[test]
fn webp_rejects_oversized_images_with_actionable_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("panorama.png");
    // Narrow but very wide: cheap to create, too wide for WebP.
    image::GrayImage::new(17_000, 2).save(&path).unwrap();
    let run = convert(
        &[path],
        OperationRequest::ImageConvert(request(FileFormat::Webp)),
        &settings(CollisionPolicy::Rename),
    );
    let TaskOutcome::Failed { error } = task_outcomes(&run).remove(0) else {
        panic!()
    };
    assert_eq!(
        error.code,
        omnidrop_engine::ErrorCode::TargetDimensionsUnsupported
    );
    assert!(no_temp_files(dir.path()));
}

#[test]
fn animated_gif_converts_first_frame_with_note() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("anim.gif");
    {
        let file = std::fs::File::create(&path).unwrap();
        let mut encoder = image::codecs::gif::GifEncoder::new(file);
        for shade in [10u8, 250] {
            let frame = image::Frame::new(image::RgbaImage::from_pixel(
                8,
                8,
                image::Rgba([shade, 0, 0, 255]),
            ));
            encoder.encode_frame(frame).unwrap();
        }
    }
    let run = convert(
        &[path],
        OperationRequest::ImageConvert(request(FileFormat::Png)),
        &settings(CollisionPolicy::Rename),
    );
    let TaskOutcome::Succeeded { notes, .. } = task_outcomes(&run).remove(0) else {
        panic!()
    };
    assert!(notes.contains(&TaskNote::FirstFrameOnly));
}

pub fn task_outcomes(run: &Run) -> Vec<TaskOutcome> {
    run.events
        .iter()
        .filter_map(|e| match e {
            omnidrop_engine::JobEvent::TaskFinished { outcome, .. } => Some(outcome.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn every_task_reports_intermediate_progress() {
    let dir = tempfile::tempdir().unwrap();
    let paths: Vec<PathBuf> = (0..3)
        .map(|i| {
            let p = dir.path().join(format!("{i}.png"));
            write_png(&p, 64, 64, false);
            p
        })
        .collect();
    let run = convert(
        &paths,
        OperationRequest::ImageConvert(request(FileFormat::Png)),
        &settings(CollisionPolicy::Rename),
    );
    for task in run.events.iter().filter_map(|e| match e {
        omnidrop_engine::JobEvent::TaskStarted { task_id, .. } => Some(task_id),
        _ => None,
    }) {
        let middle = run.events.iter().any(|e| {
            matches!(e, omnidrop_engine::JobEvent::TaskProgress { task_id, progress, .. }
                if task_id == task && *progress > 0.2 && *progress < 1.0)
        });
        assert!(middle, "task {task} only reported 0% and 100%");
    }
}
