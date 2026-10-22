mod common;

use common::*;
use omnidrop_engine::job::{JobEvent, TaskOutcome};
use omnidrop_engine::operation::{
    CollisionPolicy, PdfMergeOptions, PdfRotateOptions, PdfSplitOptions, PdfToImagesOptions, Preset,
};
use omnidrop_engine::{ErrorCode, FileDetails, FileFormat, OperationRequest, ToolSet};

#[test]
fn inspector_counts_pages() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("doc.pdf");
    write_pdf(&path, 3);
    let files = inspect_all(&[path], &ToolSet::none());
    assert_eq!(files[0].format, Some(FileFormat::Pdf));
    assert_eq!(files[0].details, Some(FileDetails::Pdf { pages: 3 }));
}

#[test]
fn merge_keeps_page_order_and_inherited_resources() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.pdf");
    let b = dir.path().join("b.pdf");
    write_pdf(&a, 2);
    write_pdf(&b, 3);
    let run = convert(
        &[a, b],
        OperationRequest::PdfMerge(PdfMergeOptions {
            output_name: Some("Combined".into()),
        }),
        &settings(CollisionPolicy::Rename),
    );
    assert_eq!(run.summary.succeeded, 1, "{:?}", run.events);
    let merged = &run.summary.outputs[0].path;
    assert_eq!(merged, &dir.path().join("Combined.pdf"));
    assert_eq!(pdf_page_widths(merged), vec![200, 210, 200, 210, 220]);
    // Resources were inherited from the page tree; they must survive.
    let doc = lopdf::Document::load(merged).unwrap();
    for id in doc.get_pages().values() {
        assert!(doc.get_dictionary(*id).unwrap().has(b"Resources"));
    }
}

#[test]
fn split_each_page_and_by_ranges() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Report.pdf");
    write_pdf(&path, 4);
    let run = convert(
        std::slice::from_ref(&path),
        OperationRequest::PdfSplit(PdfSplitOptions { ranges: None }),
        &settings(CollisionPolicy::Rename),
    );
    let folder = &run.summary.outputs[0].path;
    assert!(run.summary.outputs[0].is_dir);
    assert_eq!(folder, &dir.path().join("Report"));
    assert_eq!(
        files_in(folder),
        vec![
            "Report-p01.pdf",
            "Report-p02.pdf",
            "Report-p03.pdf",
            "Report-p04.pdf"
        ]
    );
    assert_eq!(pdf_page_widths(&folder.join("Report-p03.pdf")), vec![220]);

    let run = convert(
        &[path],
        OperationRequest::PdfSplit(PdfSplitOptions {
            ranges: Some("1-2, 4".into()),
        }),
        &settings(CollisionPolicy::Rename),
    );
    let folder = &run.summary.outputs[0].path;
    assert_eq!(folder, &dir.path().join("Report (1)"));
    assert_eq!(
        files_in(folder),
        vec!["Report-p01-02.pdf", "Report-p04.pdf"]
    );
    assert_eq!(
        pdf_page_widths(&folder.join("Report-p01-02.pdf")),
        vec![200, 210]
    );
}

#[test]
fn rotate_selected_pages() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scan.pdf");
    write_pdf(&path, 3);
    let run = convert(
        std::slice::from_ref(&path),
        OperationRequest::PdfRotate(PdfRotateOptions {
            angle: 90,
            pages: Some("2-3".into()),
        }),
        &settings(CollisionPolicy::Overwrite),
    );
    let out = &run.summary.outputs[0].path;
    // Even with "overwrite", the original is never replaced.
    assert_eq!(out, &dir.path().join("scan (1).pdf"));
    assert_eq!(pdf_rotations(out), vec![0, 90, 90]);
    assert_eq!(pdf_rotations(&path), vec![0, 0, 0]);

    let run = convert(
        std::slice::from_ref(out),
        OperationRequest::PdfRotate(PdfRotateOptions {
            angle: 270,
            pages: None,
        }),
        &settings(CollisionPolicy::Rename),
    );
    assert_eq!(pdf_rotations(&run.summary.outputs[0].path), vec![270, 0, 0]);
}

#[test]
fn pages_render_to_images() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("slides.pdf");
    write_pdf(&path, 3);
    for (target, preset, ext) in [
        (FileFormat::Png, Preset::Smallest, "png"),
        (FileFormat::Jpeg, Preset::Balanced, "jpg"),
        (FileFormat::Webp, Preset::Balanced, "webp"),
    ] {
        let run = convert(
            std::slice::from_ref(&path),
            OperationRequest::PdfToImages(PdfToImagesOptions {
                target,
                preset,
                pages: Some("1, 3".into()),
            }),
            &settings(CollisionPolicy::Rename),
        );
        assert_eq!(run.summary.succeeded, 1, "{:?}", run.events);
        let folder = &run.summary.outputs[0].path;
        let names = files_in(folder);
        assert_eq!(
            names,
            vec![format!("slides-p01.{ext}"), format!("slides-p03.{ext}")]
        );
        let first = image::open(folder.join(&names[0])).unwrap();
        let dpi = if preset == Preset::Smallest {
            96.0
        } else {
            150.0
        };
        let expected_width = (200.0 * dpi / 72.0) as u32;
        assert!(
            first.width().abs_diff(expected_width) <= 1,
            "{} vs {expected_width}",
            first.width()
        );
        // Pages render on white and contain dark text.
        let rgb = first.to_rgb8();
        assert_eq!(rgb.get_pixel(0, 0).0, [255, 255, 255]);
        assert!(rgb.pixels().any(|p| p[0] < 100), "text should be visible");
    }
}

#[test]
fn corrupt_pdf_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("broken.pdf");
    std::fs::write(&path, b"%PDF-1.7\n1 0 obj << /Type /Catalog >> garbage").unwrap();
    let files = inspect_all(&[path], &ToolSet::none());
    assert_eq!(files[0].format, Some(FileFormat::Pdf));
    assert!(files[0].issue.is_some());
    let err = omnidrop_engine::plan(
        &files,
        &OperationRequest::PdfMerge(PdfMergeOptions::default()),
        &settings(CollisionPolicy::Rename),
        &omnidrop_engine::CapabilityRegistry::new(&ToolSet::none()),
    )
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::NotEnoughFiles);
}

#[test]
fn password_protected_pdf_is_explained() {
    let dir = tempfile::tempdir().unwrap();
    let plain = dir.path().join("plain.pdf");
    write_pdf(&plain, 1);
    let locked = dir.path().join("locked.pdf");
    let mut doc = lopdf::Document::load(&plain).unwrap();
    let file_id = lopdf::Object::string_literal(b"0123456789abcdef".to_vec());
    doc.trailer.set("ID", vec![file_id.clone(), file_id]);
    let state = lopdf::EncryptionState::try_from(lopdf::EncryptionVersion::V2 {
        document: &doc,
        owner_password: "owner",
        user_password: "secret",
        key_length: 128,
        permissions: lopdf::Permissions::all(),
    })
    .unwrap();
    doc.encrypt(&state).unwrap();
    doc.save(&locked).unwrap();

    let files = inspect_all(std::slice::from_ref(&locked), &ToolSet::none());
    let code = files[0].issue.as_ref().map(|i| i.code);
    assert_eq!(code, Some(ErrorCode::PdfEncrypted));
    let run_err = omnidrop_engine::executors::pdf::load(&locked, u64::MAX).unwrap_err();
    assert_eq!(run_err.code, ErrorCode::PdfEncrypted);
    let _ = JobEvent::JobStarted {
        job_id: String::new(),
        total: 0,
    };
    let _ = TaskOutcome::Cancelled;
}

#[test]
fn owner_password_only_pdf_is_processed() {
    let dir = tempfile::tempdir().unwrap();
    let plain = dir.path().join("plain.pdf");
    write_pdf(&plain, 2);
    let restricted = dir.path().join("restricted.pdf");
    let mut doc = lopdf::Document::load(&plain).unwrap();
    let file_id = lopdf::Object::string_literal(b"fedcba9876543210".to_vec());
    doc.trailer.set("ID", vec![file_id.clone(), file_id]);
    let state = lopdf::EncryptionState::try_from(lopdf::EncryptionVersion::V2 {
        document: &doc,
        owner_password: "owner",
        user_password: "",
        key_length: 128,
        permissions: lopdf::Permissions::PRINTABLE,
    })
    .unwrap();
    doc.encrypt(&state).unwrap();
    doc.save(&restricted).unwrap();

    let files = inspect_all(std::slice::from_ref(&restricted), &ToolSet::none());
    assert!(files[0].issue.is_none(), "{:?}", files[0].issue);
    assert_eq!(files[0].details, Some(FileDetails::Pdf { pages: 2 }));
    let run = convert(
        &[restricted],
        OperationRequest::PdfRotate(PdfRotateOptions {
            angle: 180,
            pages: None,
        }),
        &settings(CollisionPolicy::Rename),
    );
    assert_eq!(run.summary.succeeded, 1, "{:?}", run.events);
    assert_eq!(pdf_rotations(&run.summary.outputs[0].path), vec![180, 180]);
}
