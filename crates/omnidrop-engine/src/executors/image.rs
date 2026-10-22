//! Image conversion: PNG, JPEG, WebP, TIFF, BMP, GIF and ICO in; PNG, JPEG,
//! WebP, AVIF, TIFF and BMP out.

use super::Produced;
use crate::error::{EngineError, EngineResult, ErrorCode};
use crate::format::FileFormat;
use crate::job::{TaskContext, preset_quality};
use crate::operation::{ImageConvertOptions, Preset, ResizeSpec};
use crate::output::Staged;
use crate::plan::{TaskNote, TaskSpec, parse_hex_color};
use crate::validator::Expectation;
use image::codecs::avif::AvifEncoder;
use image::codecs::bmp::BmpEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::codecs::tiff::TiffEncoder;
use image::imageops::FilterType;
use image::{
    ColorType, DynamicImage, GenericImageView, ImageDecoder, ImageEncoder, ImageError, RgbImage,
};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;

/// WebP cannot store images wider or taller than this.
pub const WEBP_MAX_SIDE: u32 = 16_383;
/// JPEG cannot store images wider or taller than this.
pub const JPEG_MAX_SIDE: u32 = 65_535;

pub fn convert(
    options: &ImageConvertOptions,
    task: &TaskSpec,
    ctx: &TaskContext<'_>,
) -> EngineResult<Produced> {
    let input = &task.inputs[0];
    if input.size > ctx.limits.max_image_file_bytes {
        return Err(EngineError::new(
            ErrorCode::InputTooLarge,
            "image file too large",
        ));
    }
    ctx.cancel.check()?;
    let decoded = decode(&input.path, ctx)?;
    ctx.progress(0.35);
    ctx.cancel.check()?;

    let image = resize(decoded.image, options.resize);
    ctx.progress(0.5);
    ctx.cancel.check()?;

    let mut notes = Vec::new();
    if decoded.animated {
        notes.push(TaskNote::FirstFrameOnly);
    }
    let settings = EncodeSettings {
        quality: options
            .quality
            .unwrap_or_else(|| preset_quality(options.target, options.preset)),
        lossless: options.lossless,
        preset: options.preset,
        background: options
            .background
            .as_deref()
            .and_then(parse_hex_color)
            .unwrap_or([255, 255, 255]),
        icc: decoded.icc.as_deref(),
    };
    let target = task.output();
    let mut temp = ctx.output.temp_file(&target.dir)?;
    encode(
        &image,
        options.target,
        &settings,
        temp.as_file_mut(),
        &mut notes,
    )?;
    ctx.progress(0.9);

    Ok(Produced {
        staged: Staged::File(temp),
        expectation: Expectation::Image {
            format: options.target,
            width: image.width(),
            height: image.height(),
        },
        notes,
    })
}

struct Decoded {
    image: DynamicImage,
    icc: Option<Vec<u8>>,
    animated: bool,
}

fn decode(path: &Path, ctx: &TaskContext<'_>) -> EngineResult<Decoded> {
    let file = File::open(path).map_err(|e| EngineError::read(&e))?;
    let mut reader = image::ImageReader::new(BufReader::new(file))
        .with_guessed_format()
        .map_err(|e| EngineError::read(&e))?;
    let format = reader.format();
    reader.limits(ctx.limits.image_limits());
    let mut decoder = reader.into_decoder().map_err(|e| map_image_error(&e))?;
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let icc = decoder.icc_profile().ok().flatten();
    let mut image = DynamicImage::from_decoder(decoder).map_err(|e| map_image_error(&e))?;
    image.apply_orientation(orientation);
    let animated = format == Some(image::ImageFormat::Gif)
        && crate::inspector::gif_frame_count(path, 2).unwrap_or(1) > 1;
    Ok(Decoded {
        image,
        icc,
        animated,
    })
}

pub fn map_image_error(err: &ImageError) -> EngineError {
    match err {
        ImageError::Limits(_) => EngineError::new(ErrorCode::DimensionsTooLarge, err.to_string()),
        ImageError::Unsupported(_) => {
            EngineError::new(ErrorCode::UnsupportedFormat, err.to_string())
        }
        ImageError::IoError(io) => EngineError::read(io),
        _ => EngineError::new(ErrorCode::Unreadable, err.to_string()),
    }
}

/// Size after applying `spec` to a `width`×`height` image.
pub fn target_dimensions(width: u32, height: u32, spec: Option<ResizeSpec>) -> (u32, u32) {
    match spec {
        None => (width, height),
        Some(ResizeSpec::Fit {
            max_width,
            max_height,
        }) => {
            let max_w = max_width.unwrap_or(u32::MAX).max(1);
            let max_h = max_height.unwrap_or(u32::MAX).max(1);
            if width <= max_w && height <= max_h {
                return (width, height);
            }
            let scale =
                (f64::from(max_w) / f64::from(width)).min(f64::from(max_h) / f64::from(height));
            (
                ((f64::from(width) * scale).round() as u32).clamp(1, max_w),
                ((f64::from(height) * scale).round() as u32).clamp(1, max_h),
            )
        }
        Some(ResizeSpec::Percent { percent }) => {
            let scale =
                |side: u32| ((u64::from(side) * u64::from(percent) + 50) / 100).max(1) as u32;
            (scale(width), scale(height))
        }
    }
}

fn resize(image: DynamicImage, spec: Option<ResizeSpec>) -> DynamicImage {
    let (width, height) = image.dimensions();
    let (new_w, new_h) = target_dimensions(width, height, spec);
    if (new_w, new_h) == (width, height) {
        image
    } else {
        image.resize_exact(new_w, new_h, FilterType::Lanczos3)
    }
}

pub struct EncodeSettings<'a> {
    pub quality: u8,
    pub lossless: bool,
    pub preset: Preset,
    pub background: [u8; 3],
    pub icc: Option<&'a [u8]>,
}

/// Encodes `image` as `target` into `out`, adding notes about anything that
/// could not be carried over.
pub fn encode(
    image: &DynamicImage,
    target: FileFormat,
    settings: &EncodeSettings<'_>,
    out: &mut File,
    notes: &mut Vec<TaskNote>,
) -> EngineResult<()> {
    let (width, height) = image.dimensions();
    let max_side = match target {
        FileFormat::Webp => WEBP_MAX_SIDE,
        FileFormat::Jpeg => JPEG_MAX_SIDE,
        _ => u32::MAX,
    };
    if width > max_side || height > max_side {
        return Err(EngineError::new(
            ErrorCode::TargetDimensionsUnsupported,
            format!("{target:?} supports at most {max_side} pixels per side"),
        ));
    }

    let mut writer = BufWriter::new(out);
    let mut icc_kept = settings.icc.is_none();
    let encode_error = |e: ImageError| match e {
        ImageError::IoError(io) => EngineError::write(&io),
        other => EngineError::internal(format!("encoder failed: {other}")),
    };
    match target {
        FileFormat::Png => {
            let compression = match settings.preset {
                Preset::Smallest => CompressionType::Best,
                Preset::Fastest => CompressionType::Fast,
                _ => CompressionType::Default,
            };
            let mut encoder =
                PngEncoder::new_with_quality(&mut writer, compression, PngFilter::Adaptive);
            icc_kept |= apply_icc(&mut encoder, settings.icc);
            lossless_compatible(image)
                .write_with_encoder(encoder)
                .map_err(encode_error)?;
        }
        FileFormat::Tiff => {
            let mut encoder = TiffEncoder::new(&mut writer);
            icc_kept |= apply_icc(&mut encoder, settings.icc);
            tiff_compatible(image)
                .write_with_encoder(encoder)
                .map_err(encode_error)?;
        }
        FileFormat::Jpeg => {
            let mut encoder =
                JpegEncoder::new_with_quality(&mut writer, settings.quality.clamp(1, 100));
            icc_kept |= apply_icc(&mut encoder, settings.icc);
            let opaque = match image.color() {
                ColorType::L8 | ColorType::L16 => DynamicImage::ImageLuma8(image.to_luma8()),
                color if color.has_alpha() => {
                    let (flat, had_transparency) = flatten(image, settings.background);
                    if had_transparency {
                        notes.push(TaskNote::TransparencyFlattened);
                    }
                    DynamicImage::ImageRgb8(flat)
                }
                _ => DynamicImage::ImageRgb8(image.to_rgb8()),
            };
            opaque.write_with_encoder(encoder).map_err(encode_error)?;
        }
        FileFormat::Bmp => {
            let encoder = BmpEncoder::new(&mut writer);
            eight_bit(image)
                .write_with_encoder(encoder)
                .map_err(encode_error)?;
        }
        FileFormat::Avif => {
            let speed = match settings.preset {
                Preset::Smallest | Preset::Quality => 6,
                _ => 8,
            };
            let encoder = AvifEncoder::new_with_speed_quality(
                &mut writer,
                speed,
                settings.quality.clamp(1, 100),
            );
            eight_bit(image)
                .write_with_encoder(encoder)
                .map_err(encode_error)?;
        }
        FileFormat::Webp => {
            let memory = if image.color().has_alpha() {
                let rgba = image.to_rgba8();
                webp::Encoder::from_rgba(rgba.as_raw(), width, height)
                    .encode_simple(settings.lossless, f32::from(settings.quality))
            } else {
                let rgb = image.to_rgb8();
                webp::Encoder::from_rgb(rgb.as_raw(), width, height)
                    .encode_simple(settings.lossless, f32::from(settings.quality))
            }
            .map_err(|e| EngineError::internal(format!("WebP encoder failed: {e:?}")))?;
            writer
                .write_all(&memory)
                .map_err(|e| EngineError::write(&e))?;
        }
        other => {
            return Err(EngineError::new(
                ErrorCode::UnsupportedConversion,
                format!("cannot write {other:?} images"),
            ));
        }
    }
    writer.flush().map_err(|e| EngineError::write(&e))?;
    if !icc_kept {
        notes.push(TaskNote::ColorProfileDropped);
    }
    Ok(())
}

/// Embeds an ICC profile when the encoder supports it. Returns whether it was kept.
fn apply_icc(encoder: &mut impl ImageEncoder, icc: Option<&[u8]>) -> bool {
    icc.is_some_and(|profile| encoder.set_icc_profile(profile.to_vec()).is_ok())
}

/// Fills transparent pixels with `background`. Also reports whether any pixel
/// was not fully opaque.
pub fn flatten(image: &DynamicImage, background: [u8; 3]) -> (RgbImage, bool) {
    let rgba = image.to_rgba8();
    let mut out = RgbImage::new(rgba.width(), rgba.height());
    let mut had_transparency = false;
    for (dst, src) in out.pixels_mut().zip(rgba.pixels()) {
        let alpha = u32::from(src[3]);
        if alpha < 255 {
            had_transparency = true;
        }
        for channel in 0..3 {
            let value =
                u32::from(src[channel]) * alpha + u32::from(background[channel]) * (255 - alpha);
            dst[channel] = ((value + 127) / 255) as u8;
        }
    }
    (out, had_transparency)
}

fn lossless_compatible(image: &DynamicImage) -> DynamicImage {
    match image.color() {
        ColorType::L8
        | ColorType::La8
        | ColorType::Rgb8
        | ColorType::Rgba8
        | ColorType::L16
        | ColorType::La16
        | ColorType::Rgb16
        | ColorType::Rgba16 => image.clone(),
        ColorType::Rgb32F => DynamicImage::ImageRgb16(image.to_rgb16()),
        _ => DynamicImage::ImageRgba16(image.to_rgba16()),
    }
}

fn tiff_compatible(image: &DynamicImage) -> DynamicImage {
    match image.color() {
        ColorType::L8
        | ColorType::Rgb8
        | ColorType::Rgba8
        | ColorType::L16
        | ColorType::Rgb16
        | ColorType::Rgba16 => image.clone(),
        ColorType::La8 => DynamicImage::ImageRgba8(image.to_rgba8()),
        ColorType::Rgb32F => DynamicImage::ImageRgb16(image.to_rgb16()),
        _ => DynamicImage::ImageRgba16(image.to_rgba16()),
    }
}

fn eight_bit(image: &DynamicImage) -> DynamicImage {
    if image.color().has_alpha() {
        DynamicImage::ImageRgba8(image.to_rgba8())
    } else {
        DynamicImage::ImageRgb8(image.to_rgb8())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_never_enlarges_and_keeps_ratio() {
        let fit = |w, h| {
            Some(ResizeSpec::Fit {
                max_width: w,
                max_height: h,
            })
        };
        assert_eq!(
            target_dimensions(4000, 3000, fit(Some(1920), None)),
            (1920, 1440)
        );
        assert_eq!(
            target_dimensions(4000, 3000, fit(None, Some(300))),
            (400, 300)
        );
        assert_eq!(
            target_dimensions(100, 50, fit(Some(1920), Some(1080))),
            (100, 50)
        );
        assert_eq!(
            target_dimensions(3000, 4000, fit(Some(1000), Some(1000))),
            (750, 1000)
        );
        assert_eq!(target_dimensions(10_000, 1, fit(Some(10), None)), (10, 1));
    }

    #[test]
    fn percent_scaling() {
        let pct = |p| Some(ResizeSpec::Percent { percent: p });
        assert_eq!(target_dimensions(200, 100, pct(50)), (100, 50));
        assert_eq!(target_dimensions(3, 3, pct(1)), (1, 1));
        assert_eq!(target_dimensions(10, 10, pct(250)), (25, 25));
    }

    #[test]
    fn flatten_blends_on_background() {
        let mut rgba = image::RgbaImage::new(2, 1);
        rgba.put_pixel(0, 0, image::Rgba([0, 0, 0, 0]));
        rgba.put_pixel(1, 0, image::Rgba([0, 0, 0, 255]));
        let (flat, transparent) = flatten(&DynamicImage::ImageRgba8(rgba), [255, 255, 255]);
        assert!(transparent);
        assert_eq!(flat.get_pixel(0, 0).0, [255, 255, 255]);
        assert_eq!(flat.get_pixel(1, 0).0, [0, 0, 0]);
    }
}
