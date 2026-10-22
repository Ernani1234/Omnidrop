//! PDF operations: merge, split and rotate with lopdf; rendering pages to
//! images with hayro (pure Rust, no external binaries).

use super::Produced;
use super::image::{EncodeSettings, encode};
use crate::error::{EngineError, EngineResult, ErrorCode};
use crate::format::FileFormat;
use crate::job::{TaskContext, preset_quality};
use crate::names::stem_of;
use crate::operation::{PdfRotateOptions, PdfSplitOptions, PdfToImagesOptions, Preset};
use crate::output::Staged;
use crate::pages;
use crate::plan::{TaskNote, TaskSpec};
use crate::validator::{Expectation, FolderItem};
use lopdf::{Dictionary, Document, Object, ObjectId, dictionary};
use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Attributes a page may inherit from its ancestors in the page tree.
const INHERITABLE: [&[u8]; 4] = [b"Resources", b"MediaBox", b"CropBox", b"Rotate"];
/// Streams inside a PDF may not inflate beyond this while loading.
const MAX_DECOMPRESSED_STREAM: usize = 512 * 1024 * 1024;

/// Counts pages without loading the whole document.
pub fn count_pages(data: Vec<u8>) -> EngineResult<u32> {
    use hayro::hayro_syntax::{LoadPdfError, Pdf};
    match Pdf::new(data) {
        Ok(pdf) => Ok(pdf.pages().len() as u32),
        Err(LoadPdfError::Decryption(e)) => {
            Err(EngineError::new(ErrorCode::PdfEncrypted, format!("{e:?}")))
        }
        Err(LoadPdfError::Invalid) => Err(EngineError::new(ErrorCode::Unreadable, "invalid PDF")),
    }
}

/// Loads a PDF with lopdf, decrypting documents that only have an owner
/// password. Documents that need a password to open are rejected.
pub fn load(path: &Path, max_bytes: u64) -> EngineResult<Document> {
    let size = std::fs::metadata(path)
        .map_err(|e| EngineError::read(&e))?
        .len();
    if size > max_bytes {
        return Err(EngineError::new(
            ErrorCode::InputTooLarge,
            "PDF exceeds size limit",
        ));
    }
    let options = lopdf::LoadOptions {
        max_decompressed_size: Some(MAX_DECOMPRESSED_STREAM),
        ..Default::default()
    };
    let mut doc = Document::load_with_options(path, options).map_err(map_lopdf_error)?;
    if doc.is_encrypted() {
        doc.decrypt("").map_err(|e| {
            EngineError::new(ErrorCode::PdfEncrypted, format!("password required: {e}"))
        })?;
    }
    Ok(doc)
}

fn map_lopdf_error(err: lopdf::Error) -> EngineError {
    match err {
        lopdf::Error::Decryption(_) | lopdf::Error::InvalidPassword => {
            EngineError::new(ErrorCode::PdfEncrypted, err.to_string())
        }
        lopdf::Error::IO(io) => EngineError::read(&io),
        other => EngineError::new(ErrorCode::Unreadable, other.to_string()),
    }
}

fn save(doc: &mut Document, file: &mut File) -> EngineResult<()> {
    let mut writer = BufWriter::new(file);
    doc.save_to(&mut writer)
        .map_err(|e| EngineError::write(&e))?;
    writer.flush().map_err(|e| EngineError::write(&e))
}

/// Builds a new document from pages of existing ones, copying only the
/// objects those pages reference.
struct Assembler {
    out: Document,
    kids: Vec<ObjectId>,
}

impl Assembler {
    fn new() -> Self {
        Self {
            out: Document::with_version("1.7"),
            kids: Vec::new(),
        }
    }

    /// Copies `page_ids` (in order) from `doc`. Object ids of `doc` must not
    /// collide with ids already copied (renumber before merging).
    fn add_pages(&mut self, doc: &Document, page_ids: &[ObjectId]) {
        let selected: HashSet<ObjectId> = page_ids.iter().copied().collect();
        let mut queue: Vec<ObjectId> = page_ids.to_vec();
        let mut seen = HashSet::new();
        while let Some(id) = queue.pop() {
            if !seen.insert(id) || self.out.objects.contains_key(&id) {
                continue;
            }
            let Ok(object) = doc.get_object(id) else {
                continue;
            };
            let mut object = object.clone();
            if selected.contains(&id)
                && let Object::Dictionary(dict) = &mut object
            {
                for key in INHERITABLE {
                    if !dict.has(key)
                        && let Some(value) = inherited(doc, dict, key)
                    {
                        dict.set(key.to_vec(), value);
                    }
                }
                dict.remove(b"Parent");
            }
            let mut refs = Vec::new();
            collect_refs(&object, &mut refs);
            for target in refs {
                // Never pull in other pages or page-tree nodes (via links,
                // annotations or destinations).
                let is_page_node = doc
                    .get_object(target)
                    .ok()
                    .and_then(|o| o.type_name().ok())
                    .is_some_and(|t| t == b"Page" || t == b"Pages" || t == b"Catalog");
                if !is_page_node || selected.contains(&target) {
                    queue.push(target);
                }
            }
            self.out.max_id = self.out.max_id.max(id.0);
            self.out.objects.insert(id, object);
        }
        self.kids.extend_from_slice(page_ids);
    }

    fn finish(mut self) -> Document {
        let pages_id = self.out.new_object_id();
        for kid in &self.kids {
            if let Ok(page) = self.out.get_dictionary_mut(*kid) {
                page.set("Parent", pages_id);
            }
        }
        let kids: Vec<Object> = self.kids.iter().map(|id| Object::Reference(*id)).collect();
        let count = kids.len() as i64;
        self.out.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => kids,
                "Count" => count,
            }),
        );
        let catalog_id = self.out.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        self.out.trailer.set("Root", catalog_id);
        self.out.renumber_objects();
        self.out
    }
}

fn inherited(doc: &Document, page: &Dictionary, key: &[u8]) -> Option<Object> {
    let mut parent = page.get(b"Parent").and_then(Object::as_reference).ok();
    for _ in 0..64 {
        let id = parent?;
        let node = doc.get_dictionary(id).ok()?;
        if let Ok(value) = node.get(key) {
            return Some(value.clone());
        }
        parent = node.get(b"Parent").and_then(Object::as_reference).ok();
    }
    None
}

fn collect_refs(object: &Object, out: &mut Vec<ObjectId>) {
    match object {
        Object::Reference(id) => out.push(*id),
        Object::Array(items) => items.iter().for_each(|item| collect_refs(item, out)),
        Object::Dictionary(dict) => dict.iter().for_each(|(_, value)| collect_refs(value, out)),
        Object::Stream(stream) => stream
            .dict
            .iter()
            .for_each(|(_, value)| collect_refs(value, out)),
        _ => {}
    }
}

fn page_ids(doc: &Document) -> BTreeMap<u32, ObjectId> {
    doc.get_pages()
}

pub fn merge(task: &TaskSpec, ctx: &TaskContext<'_>) -> EngineResult<Produced> {
    let mut assembler = Assembler::new();
    let mut next_id = 1;
    let mut total_pages = 0u32;
    let count = task.inputs.len() as f32;
    for (index, input) in task.inputs.iter().enumerate() {
        ctx.cancel.check()?;
        let mut doc = load(&input.path, ctx.limits.max_pdf_bytes)?;
        doc.renumber_objects_with(next_id);
        next_id = doc.max_id + 1;
        let ids: Vec<ObjectId> = page_ids(&doc).into_values().collect();
        total_pages += ids.len() as u32;
        assembler.add_pages(&doc, &ids);
        ctx.progress(0.8 * (index + 1) as f32 / count);
    }
    ctx.cancel.check()?;
    let mut merged = assembler.finish();
    let mut temp = ctx.output.temp_file(&task.output().dir)?;
    save(&mut merged, temp.as_file_mut())?;
    Ok(Produced {
        staged: Staged::File(temp),
        expectation: Expectation::Pdf { pages: total_pages },
        notes: Vec::new(),
    })
}

pub fn split(
    options: &PdfSplitOptions,
    task: &TaskSpec,
    ctx: &TaskContext<'_>,
) -> EngineResult<Produced> {
    let input = &task.inputs[0];
    let doc = load(&input.path, ctx.limits.max_pdf_bytes)?;
    let ids = page_ids(&doc);
    let total = ids.len() as u32;
    let groups = match &options.ranges {
        Some(spec) => pages::parse_ranges(spec, total)?,
        None => (1..=total).map(|p| p..=p).collect(),
    };
    let width = total.to_string().len().max(2);
    let stem = stem_of(&input.path);
    let staging = ctx.output.temp_dir(&task.output().dir)?;
    for (index, group) in groups.iter().enumerate() {
        ctx.cancel.check()?;
        let selected: Vec<ObjectId> = group.clone().filter_map(|n| ids.get(&n).copied()).collect();
        let mut assembler = Assembler::new();
        assembler.add_pages(&doc, &selected);
        let mut part = assembler.finish();
        let name = if group.start() == group.end() {
            format!("{stem}-p{:0width$}.pdf", group.start())
        } else {
            format!(
                "{stem}-p{:0width$}-{:0width$}.pdf",
                group.start(),
                group.end()
            )
        };
        let mut file =
            File::create(staging.path().join(name)).map_err(|e| EngineError::write(&e))?;
        save(&mut part, &mut file)?;
        ctx.progress(0.9 * (index + 1) as f32 / groups.len() as f32);
    }
    Ok(Produced {
        staged: Staged::Dir(staging),
        expectation: Expectation::Folder {
            files: groups.len() as u64,
            each: Some(FolderItem::Pdf),
        },
        notes: Vec::new(),
    })
}

pub fn rotate(
    options: &PdfRotateOptions,
    task: &TaskSpec,
    ctx: &TaskContext<'_>,
) -> EngineResult<Produced> {
    let input = &task.inputs[0];
    let mut doc = load(&input.path, ctx.limits.max_pdf_bytes)?;
    let ids = page_ids(&doc);
    let total = ids.len() as u32;
    let selected = pages::selected_pages(options.pages.as_deref().unwrap_or(""), total)?;
    for number in selected {
        ctx.cancel.check()?;
        let Some(&id) = ids.get(&number) else {
            continue;
        };
        let current = {
            let page = doc.get_dictionary(id).map_err(map_lopdf_error)?;
            match page.get(b"Rotate") {
                Ok(value) => value.as_i64().unwrap_or(0),
                Err(_) => inherited(&doc, page, b"Rotate")
                    .and_then(|v| v.as_i64().ok())
                    .unwrap_or(0),
            }
        };
        let rotation = (current + i64::from(options.angle)).rem_euclid(360);
        doc.get_dictionary_mut(id)
            .map_err(map_lopdf_error)?
            .set("Rotate", rotation);
    }
    ctx.progress(0.6);
    let mut temp = ctx.output.temp_file(&task.output().dir)?;
    save(&mut doc, temp.as_file_mut())?;
    Ok(Produced {
        staged: Staged::File(temp),
        expectation: Expectation::Pdf { pages: total },
        notes: Vec::new(),
    })
}

/// Rendering resolution for each preset, in dots per inch.
pub fn preset_dpi(preset: Preset) -> f32 {
    match preset {
        Preset::Smallest => 96.0,
        Preset::Quality => 300.0,
        _ => 150.0,
    }
}

pub fn to_images(
    options: &PdfToImagesOptions,
    task: &TaskSpec,
    ctx: &TaskContext<'_>,
) -> EngineResult<Produced> {
    use hayro::hayro_interpret::InterpreterSettings;
    use hayro::hayro_syntax::{LoadPdfError, Pdf};
    use hayro::vello_cpu::color::palette::css::WHITE;
    use hayro::{PixmapSettings, RenderCache, RenderSettings};

    let input = &task.inputs[0];
    if input.size > ctx.limits.max_pdf_bytes {
        return Err(EngineError::new(
            ErrorCode::InputTooLarge,
            "PDF exceeds size limit",
        ));
    }
    let data = std::fs::read(&input.path).map_err(|e| EngineError::read(&e))?;
    let pdf = Pdf::new(data).map_err(|e| match e {
        LoadPdfError::Decryption(e) => EngineError::new(ErrorCode::PdfEncrypted, format!("{e:?}")),
        LoadPdfError::Invalid => EngineError::new(ErrorCode::Unreadable, "invalid PDF"),
    })?;
    let doc_pages = pdf.pages();
    let total = doc_pages.len() as u32;
    let selected = pages::selected_pages(options.pages.as_deref().unwrap_or(""), total)?;
    let width = total.to_string().len().max(2);
    let stem = stem_of(&input.path);
    let staging = ctx.output.temp_dir(&task.output().dir)?;
    let settings = EncodeSettings {
        quality: preset_quality(options.target, options.preset).max(85),
        lossless: false,
        preset: options.preset,
        background: [255, 255, 255],
        icc: None,
    };
    let cache = RenderCache::new();
    let interpreter = InterpreterSettings::default();
    let render_settings = RenderSettings::default();
    let mut notes = Vec::new();

    for (index, number) in selected.iter().enumerate() {
        ctx.cancel.check()?;
        let page = &doc_pages[(*number - 1) as usize];
        let (page_w, page_h) = page.render_dimensions();
        let longest = page_w.max(page_h).max(1.0);
        let mut scale = preset_dpi(options.preset) / 72.0;
        let max_side = ctx.limits.max_render_side as f32;
        if longest * scale > max_side {
            scale = max_side / longest;
            if !notes.contains(&TaskNote::ResolutionReduced) {
                notes.push(TaskNote::ResolutionReduced);
            }
        }
        let pixmap = hayro::render(
            page,
            &cache,
            &interpreter,
            &render_settings,
            &PixmapSettings {
                x_scale: scale,
                y_scale: scale,
                bg_color: WHITE,
            },
        );
        let (w, h) = (u32::from(pixmap.width()), u32::from(pixmap.height()));
        if w == 0 || h == 0 {
            return Err(EngineError::new(
                ErrorCode::Unreadable,
                format!("page {number} is empty"),
            ));
        }
        let rgba = pixmap.take_rgba8(hayro::vello_cpu::peniko::ImageAlphaType::Alpha);
        let rgba = image::RgbaImage::from_raw(w, h, rgba)
            .ok_or_else(|| EngineError::internal("rendered buffer has the wrong size"))?;
        // Pages are rendered on white, so the result is opaque.
        let image = image::DynamicImage::ImageRgb8(image::DynamicImage::ImageRgba8(rgba).to_rgb8());
        let name = format!("{stem}-p{:0width$}.{}", number, options.target.extension());
        let mut file =
            File::create(staging.path().join(name)).map_err(|e| EngineError::write(&e))?;
        let mut page_notes = Vec::new();
        encode(
            &image,
            options.target,
            &settings,
            &mut file,
            &mut page_notes,
        )?;
        ctx.progress(0.92 * (index + 1) as f32 / selected.len() as f32);
    }
    let target_format = match options.target {
        FileFormat::Jpeg => FileFormat::Jpeg,
        FileFormat::Webp => FileFormat::Webp,
        _ => FileFormat::Png,
    };
    Ok(Produced {
        staged: Staged::Dir(staging),
        expectation: Expectation::Folder {
            files: selected.len() as u64,
            each: Some(FolderItem::Image(target_format)),
        },
        notes,
    })
}
