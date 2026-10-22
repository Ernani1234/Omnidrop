mod common;

use common::*;
use omnidrop_engine::job::{JobEvent, TaskOutcome};
use omnidrop_engine::operation::{
    ArchiveCreateOptions, ArchiveExtractOptions, CollisionPolicy, Preset,
};
use omnidrop_engine::plan::TaskNote;
use omnidrop_engine::{ErrorCode, FileDetails, FileFormat, Limits, OperationRequest, ToolSet};
use std::io::Write;
use std::path::{Path, PathBuf};

fn sample_files(dir: &Path) -> Vec<PathBuf> {
    let a = dir.join("notes.txt");
    std::fs::write(&a, "hello archive\n".repeat(200)).unwrap();
    let b = dir.join("photo.png");
    write_png(&b, 32, 32, false);
    let c = dir.join("empty.dat");
    std::fs::write(&c, b"").unwrap();
    vec![a, b, c]
}

fn extract_request() -> OperationRequest {
    OperationRequest::ArchiveExtract(ArchiveExtractOptions {})
}

#[test]
fn create_and_extract_round_trip_for_every_format() {
    for format in [
        FileFormat::Zip,
        FileFormat::Tar,
        FileFormat::TarGz,
        FileFormat::SevenZ,
    ] {
        let dir = tempfile::tempdir().unwrap();
        let inputs = sample_files(dir.path());
        let run = convert(
            &inputs,
            OperationRequest::ArchiveCreate(ArchiveCreateOptions {
                target: format,
                preset: Preset::Balanced,
                output_name: Some("Bundle".into()),
            }),
            &settings(CollisionPolicy::Rename),
        );
        assert_eq!(run.summary.succeeded, 1, "{format:?}: {:?}", run.events);
        let archive = run.summary.outputs[0].path.clone();
        assert_eq!(
            archive.file_name().unwrap().to_string_lossy(),
            format!("Bundle.{}", format.extension())
        );

        let info = inspect_all(std::slice::from_ref(&archive), &ToolSet::none()).remove(0);
        assert_eq!(info.format, Some(format), "{archive:?}");
        if let Some(FileDetails::Archive { entries, .. }) = info.details {
            assert_eq!(entries, 3);
        }

        let out = tempfile::tempdir().unwrap();
        let run = convert(&[archive], extract_request(), &settings_in(out.path()));
        assert_eq!(run.summary.succeeded, 1, "{format:?}: {:?}", run.events);
        let folder = &run.summary.outputs[0].path;
        assert_eq!(folder, &out.path().join("Bundle"));
        for input in &inputs {
            let extracted = folder.join(input.file_name().unwrap());
            assert_eq!(
                std::fs::read(&extracted).unwrap(),
                std::fs::read(input).unwrap(),
                "{format:?} {extracted:?}"
            );
        }
    }
}

#[test]
fn archive_keeps_imported_folder_structure() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Project");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("README.md"), "# hi").unwrap();
    std::fs::write(root.join("src").join("main.rs"), "fn main() {}").unwrap();
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
    let request = OperationRequest::ArchiveCreate(ArchiveCreateOptions {
        target: FileFormat::Zip,
        preset: Preset::Fastest,
        output_name: None,
    });
    let plan = make_plan(&files, &request, &settings(CollisionPolicy::Rename), &tools);
    let run = run_plan(&plan, &tools, &Limits::default());
    let zip_path = &run.summary.outputs[0].path;
    assert_eq!(zip_path.file_name().unwrap(), "Project.zip");
    let mut archive = zip::ZipArchive::new(std::fs::File::open(zip_path).unwrap()).unwrap();
    let mut names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().unwrap().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, vec!["Project/README.md", "Project/src/main.rs"]);
}

fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, data) in entries {
        zip.start_file(*name, options).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn zip_slip_entries_are_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("evil.zip");
    write_zip(
        &path,
        &[
            ("../escape.txt", b"bad"),
            ("/abs/escape.txt", b"bad"),
            ("..\\win-escape.txt", b"bad"),
            ("safe/ok.txt", b"good"),
        ],
    );
    let out = tempfile::tempdir().unwrap();
    let run = convert(
        &[path],
        extract_request(),
        &settings_in(&out.path().join("target")),
    );
    let outcome = run.events.iter().find_map(|e| match e {
        JobEvent::TaskFinished { outcome, .. } => Some(outcome.clone()),
        _ => None,
    });
    let Some(TaskOutcome::Succeeded { notes, outputs }) = outcome else {
        panic!("{:?}", run.events)
    };
    assert_eq!(notes, vec![TaskNote::UnsafeEntriesSkipped { count: 3 }]);
    assert_eq!(
        std::fs::read(outputs[0].path.join("safe").join("ok.txt")).unwrap(),
        b"good"
    );
    assert!(!out.path().join("escape.txt").exists());
    assert!(!out.path().join("win-escape.txt").exists());
    assert!(!Path::new("/abs/escape.txt").exists());
}

#[test]
fn decompression_bomb_is_stopped_and_cleaned_up() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bomb.zip");
    let zeros = vec![0u8; 8 * 1024 * 1024];
    write_zip(&path, &[("a.bin", &zeros), ("b.bin", &zeros)]);
    let limits = Limits {
        archive_max_ratio: 50,
        archive_ratio_grace_bytes: 1024 * 1024,
        ..Limits::default()
    };
    let tools = ToolSet::none();
    let files = inspect_all(&[path], &tools);
    let plan = make_plan(
        &files,
        &extract_request(),
        &settings(CollisionPolicy::Rename),
        &tools,
    );
    let run = run_plan(&plan, &tools, &limits);
    let error = run.events.iter().find_map(|e| match e {
        JobEvent::TaskFinished {
            outcome: TaskOutcome::Failed { error },
            ..
        } => Some(error.code),
        _ => None,
    });
    assert_eq!(error, Some(ErrorCode::ArchiveLimitExceeded));
    assert_eq!(files_in(dir.path()), vec!["bomb.zip"]);
}

#[test]
fn entry_count_limit_applies() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("many.zip");
    let entries: Vec<(String, Vec<u8>)> =
        (0..20).map(|i| (format!("{i}.txt"), vec![b'x'])).collect();
    let refs: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(n, d)| (n.as_str(), d.as_slice()))
        .collect();
    write_zip(&path, &refs);
    let limits = Limits {
        archive_max_entries: 10,
        ..Limits::default()
    };
    let tools = ToolSet::none();
    let files = inspect_all(&[path], &tools);
    let plan = make_plan(
        &files,
        &extract_request(),
        &settings(CollisionPolicy::Rename),
        &tools,
    );
    let run = run_plan(&plan, &tools, &limits);
    assert_eq!(run.summary.failed, 1);
    assert!(no_temp_files(dir.path()));
}

#[test]
fn tar_links_are_not_extracted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("links.tar");
    {
        let mut builder = tar::Builder::new(std::fs::File::create(&path).unwrap());
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        builder
            .append_link(&mut header, "passwd", "/etc/passwd")
            .unwrap();
        let data = b"content";
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, "real.txt", &data[..])
            .unwrap();
        builder.finish().unwrap();
    }
    let out = tempfile::tempdir().unwrap();
    let run = convert(&[path], extract_request(), &settings_in(out.path()));
    let folder = &run.summary.outputs[0].path;
    assert_eq!(files_in(folder), vec!["real.txt"]);
}

#[test]
fn truncated_archive_fails_without_leftovers() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = sample_files(dir.path());
    let run = convert(
        &inputs,
        OperationRequest::ArchiveCreate(ArchiveCreateOptions {
            target: FileFormat::TarGz,
            preset: Preset::Balanced,
            output_name: Some("cut".into()),
        }),
        &settings(CollisionPolicy::Rename),
    );
    let archive = run.summary.outputs[0].path.clone();
    let bytes = std::fs::read(&archive).unwrap();
    std::fs::write(&archive, &bytes[..bytes.len() / 2]).unwrap();
    let out = tempfile::tempdir().unwrap();
    let run = convert(&[archive], extract_request(), &settings_in(out.path()));
    assert_eq!(run.summary.succeeded, 0);
    assert!(no_temp_files(out.path()));
}

#[test]
fn extracts_zip_entries_with_other_compression_methods() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mixed.zip");
    {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        for (name, method) in [
            ("bzip2.txt", zip::CompressionMethod::Bzip2),
            ("xz.txt", zip::CompressionMethod::Xz),
            ("stored.txt", zip::CompressionMethod::Stored),
        ] {
            let options = zip::write::SimpleFileOptions::default().compression_method(method);
            zip.start_file(name, options).unwrap();
            zip.write_all(format!("content of {name}\n").repeat(50).as_bytes())
                .unwrap();
        }
        zip.finish().unwrap();
    }
    let out = tempfile::tempdir().unwrap();
    let run = convert(&[path], extract_request(), &settings_in(out.path()));
    assert_eq!(run.summary.succeeded, 1, "{:?}", run.events);
    let folder = &run.summary.outputs[0].path;
    for name in ["bzip2.txt", "xz.txt", "stored.txt"] {
        let text = std::fs::read_to_string(folder.join(name)).unwrap();
        assert!(text.starts_with(&format!("content of {name}")));
    }
}
