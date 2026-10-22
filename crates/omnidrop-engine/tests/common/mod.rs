//! Shared helpers: fixtures are generated at runtime so the repository does
//! not ship binary test files.

#![allow(dead_code)]

use omnidrop_engine::inspector::{self, InputEntry};
use omnidrop_engine::job::{JobEvent, JobHandle, JobSummary, run_job};
use omnidrop_engine::operation::{CollisionPolicy, OutputLocation, OutputSettings};
use omnidrop_engine::{
    CapabilityRegistry, FileInfo, Limits, OperationRequest, Plan, ToolSet, plan,
};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub fn inspect_all(paths: &[PathBuf], tools: &ToolSet) -> Vec<FileInfo> {
    let limits = Limits::default();
    paths
        .iter()
        .enumerate()
        .map(|(i, p)| {
            inspector::inspect(
                &InputEntry {
                    path: p.clone(),
                    relative_dir: None,
                },
                format!("f{i}"),
                tools,
                &limits,
            )
        })
        .collect()
}

pub fn settings(collision: CollisionPolicy) -> OutputSettings {
    OutputSettings {
        location: OutputLocation::NextToSource,
        collision,
    }
}

pub fn settings_in(dir: &Path) -> OutputSettings {
    OutputSettings {
        location: OutputLocation::Folder {
            path: dir.to_string_lossy().into_owned(),
        },
        collision: CollisionPolicy::Rename,
    }
}

pub fn make_plan(
    files: &[FileInfo],
    request: &OperationRequest,
    settings: &OutputSettings,
    tools: &ToolSet,
) -> Plan {
    plan(files, request, settings, &CapabilityRegistry::new(tools)).expect("plan")
}

pub struct Run {
    pub summary: JobSummary,
    pub events: Vec<JobEvent>,
}

pub fn run_plan(plan: &Plan, tools: &ToolSet, limits: &Limits) -> Run {
    let handle = JobHandle::new(plan);
    run_with(plan, &handle, tools, limits, |_, _| {})
}

/// Runs a plan and lets `on_event` react (for example, cancel mid-way).
pub fn run_with(
    plan: &Plan,
    handle: &JobHandle,
    tools: &ToolSet,
    limits: &Limits,
    on_event: impl Fn(&JobEvent, &JobHandle) + Send + Sync,
) -> Run {
    let events = Mutex::new(Vec::new());
    let sink = |event: JobEvent| {
        on_event(&event, handle);
        events.lock().unwrap().push(event);
    };
    let summary = run_job(plan, handle, tools, limits, &sink);
    Run {
        summary,
        events: events.into_inner().unwrap(),
    }
}

pub fn convert(paths: &[PathBuf], request: OperationRequest, settings: &OutputSettings) -> Run {
    let tools = ToolSet::none();
    let files = inspect_all(paths, &tools);
    let plan = make_plan(&files, &request, settings, &tools);
    run_plan(&plan, &tools, &Limits::default())
}

pub fn write_png(path: &Path, width: u32, height: u32, alpha: bool) {
    if alpha {
        let img = image::RgbaImage::from_fn(width, height, |x, y| {
            image::Rgba([
                (x * 7) as u8,
                (y * 5) as u8,
                120,
                if x < width / 2 { 0 } else { 255 },
            ])
        });
        img.save_with_format(path, image::ImageFormat::Png).unwrap();
    } else {
        let img = image::RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x * 3) as u8, (y * 3) as u8, 80])
        });
        img.save_with_format(path, image::ImageFormat::Png).unwrap();
    }
}

pub fn write_image(path: &Path, format: image::ImageFormat, width: u32, height: u32) {
    let img = image::RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x * 9) as u8, (y * 4) as u8, 200])
    });
    match format {
        // Icons embed RGBA images.
        image::ImageFormat::Ico => {
            image::DynamicImage::ImageRgba8(image::DynamicImage::ImageRgb8(img).to_rgba8())
                .resize_exact(
                    width.min(256),
                    height.min(256),
                    image::imageops::FilterType::Nearest,
                )
                .save_with_format(path, format)
                .unwrap()
        }
        _ => image::DynamicImage::ImageRgb8(img)
            .save_with_format(path, format)
            .unwrap(),
    }
}

/// Writes a simple PDF with `pages` pages, each with a different size so
/// page order can be checked.
pub fn write_pdf(path: &Path, pages: u32) {
    use lopdf::content::{Content, Operation};
    use lopdf::{Document, Object, Stream, dictionary};
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let resources_id = doc.add_object(dictionary! { "Font" => dictionary! { "F1" => font_id } });
    let mut kids = vec![];
    for i in 0..pages {
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 24.into()]),
                Operation::new("Td", vec![50.into(), 100.into()]),
                Operation::new(
                    "Tj",
                    vec![Object::string_literal(format!("Page {}", i + 1))],
                ),
                Operation::new("ET", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let width = 200 + i64::from(i) * 10;
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "MediaBox" => vec![0.into(), 0.into(), width.into(), 300.into()],
        });
        kids.push(Object::Reference(page_id));
    }
    // Resources are inherited from the page tree on purpose.
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => i64::from(pages),
            "Resources" => resources_id,
        }),
    );
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    doc.save(path).unwrap();
}

/// Widths of the pages of a PDF, in order (fixtures encode page order in width).
pub fn pdf_page_widths(path: &Path) -> Vec<i64> {
    let doc = lopdf::Document::load(path).unwrap();
    doc.get_pages()
        .values()
        .map(|id| {
            let page = doc.get_dictionary(*id).unwrap();
            let media = page.get(b"MediaBox").unwrap().as_array().unwrap();
            media[2]
                .as_i64()
                .unwrap_or_else(|_| media[2].as_float().unwrap() as i64)
        })
        .collect()
}

pub fn pdf_rotations(path: &Path) -> Vec<i64> {
    let doc = lopdf::Document::load(path).unwrap();
    doc.get_pages()
        .values()
        .map(|id| {
            doc.get_dictionary(*id)
                .unwrap()
                .get(b"Rotate")
                .and_then(|r| r.as_i64())
                .unwrap_or(0)
        })
        .collect()
}

pub fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

pub fn no_temp_files(dir: &Path) -> bool {
    walk(dir).iter().all(|p| {
        !p.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(".omnidrop-")
    })
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    for entry in std::fs::read_dir(dir).unwrap().filter_map(Result::ok) {
        let path = entry.path();
        out.push(path.clone());
        if path.is_dir() {
            out.extend(walk(&path));
        }
    }
    out
}
