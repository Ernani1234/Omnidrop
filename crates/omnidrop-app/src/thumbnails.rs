//! Small previews for the queue, returned as PNG data URLs so the webview
//! never needs direct file access.

use base64::Engine;
use omnidrop_engine::{FileFormat, FileInfo, FormatFamily, Limits};
use std::io::Cursor;

pub const THUMBNAIL_SIDE: u32 = 112;
const MAX_IMAGE_BYTES: u64 = 80 * 1024 * 1024;
const MAX_PDF_BYTES: u64 = 60 * 1024 * 1024;

pub fn make(file: &FileInfo, limits: &Limits) -> Option<String> {
    if file.issue.is_some() {
        return None;
    }
    let image = match (file.family?, file.format?) {
        // AVIF cannot be decoded without native libraries.
        (FormatFamily::Image, FileFormat::Avif) => return None,
        (FormatFamily::Image, _) if file.size <= MAX_IMAGE_BYTES => image_thumbnail(file, limits)?,
        (FormatFamily::Pdf, _) if file.size <= MAX_PDF_BYTES => pdf_thumbnail(file)?,
        _ => return None,
    };
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    ))
}

fn image_thumbnail(file: &FileInfo, limits: &Limits) -> Option<image::DynamicImage> {
    use image::ImageDecoder;
    let mut reader = image::ImageReader::open(&file.path)
        .ok()?
        .with_guessed_format()
        .ok()?;
    reader.limits(limits.image_limits());
    let mut decoder = reader.into_decoder().ok()?;
    let orientation = decoder.orientation().ok();
    let mut image = image::DynamicImage::from_decoder(decoder).ok()?;
    if let Some(orientation) = orientation {
        image.apply_orientation(orientation);
    }
    Some(image.thumbnail(THUMBNAIL_SIDE, THUMBNAIL_SIDE))
}

fn pdf_thumbnail(file: &FileInfo) -> Option<image::DynamicImage> {
    use hayro::hayro_interpret::InterpreterSettings;
    use hayro::hayro_syntax::Pdf;
    use hayro::vello_cpu::color::palette::css::WHITE;
    use hayro::{PixmapSettings, RenderCache, RenderSettings};

    let data = std::fs::read(&file.path).ok()?;
    let pdf = Pdf::new(data).ok()?;
    let page = pdf.pages().first()?;
    let (w, h) = page.render_dimensions();
    let scale = THUMBNAIL_SIDE as f32 / w.max(h).max(1.0);
    let pixmap = hayro::render(
        page,
        &RenderCache::new(),
        &InterpreterSettings::default(),
        &RenderSettings::default(),
        &PixmapSettings {
            x_scale: scale,
            y_scale: scale,
            bg_color: WHITE,
        },
    );
    let (pw, ph) = (u32::from(pixmap.width()), u32::from(pixmap.height()));
    let rgba = pixmap.take_rgba8(hayro::vello_cpu::peniko::ImageAlphaType::Alpha);
    image::RgbaImage::from_raw(pw, ph, rgba).map(image::DynamicImage::ImageRgba8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omnidrop_engine::ToolSet;
    use omnidrop_engine::inspector::{InputEntry, inspect};

    #[test]
    fn makes_png_data_urls_for_images() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.png");
        image::RgbImage::new(640, 320).save(&path).unwrap();
        let limits = Limits::default();
        let info = inspect(
            &InputEntry {
                path,
                relative_dir: None,
            },
            "1".into(),
            &ToolSet::none(),
            &limits,
        );
        let url = make(&info, &limits).expect("thumbnail");
        assert!(url.starts_with("data:image/png;base64,"));

        let text = dir.path().join("notes.txt");
        std::fs::write(&text, "hello").unwrap();
        let info = inspect(
            &InputEntry {
                path: text,
                relative_dir: None,
            },
            "2".into(),
            &ToolSet::none(),
            &limits,
        );
        assert_eq!(make(&info, &limits), None);
    }
}
