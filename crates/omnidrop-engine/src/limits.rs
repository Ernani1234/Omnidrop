//! Resource limits that protect the machine from oversized or malicious input.

use std::time::Duration;

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;

#[derive(Debug, Clone)]
pub struct Limits {
    /// Largest image file accepted for decoding.
    pub max_image_file_bytes: u64,
    /// Largest width or height accepted when decoding an image.
    pub max_image_side: u32,
    /// Upper bound for memory a single image decode may allocate.
    pub max_image_alloc_bytes: u64,
    /// Largest PDF accepted.
    pub max_pdf_bytes: u64,
    /// Largest side, in pixels, of a rendered PDF page.
    pub max_render_side: u32,
    /// Most entries an archive may contain when extracted.
    pub archive_max_entries: u64,
    /// Most bytes an archive may expand to.
    pub archive_max_total_bytes: u64,
    /// Expansion ratio above which extraction stops (decompression bombs),
    /// enforced once more than `archive_ratio_grace_bytes` were written.
    pub archive_max_ratio: u64,
    pub archive_ratio_grace_bytes: u64,
    /// Most files collected when a folder is imported.
    pub max_import_files: usize,
    /// Deepest folder level visited during import.
    pub max_import_depth: usize,
    /// Minimum time budget for one media conversion.
    pub media_min_timeout: Duration,
    /// Time budget per second of media (applied when duration is known).
    pub media_timeout_factor: u32,
    /// Absolute cap for one media conversion.
    pub media_max_timeout: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_image_file_bytes: 2 * GIB,
            max_image_side: 40_000,
            max_image_alloc_bytes: 3 * GIB,
            max_pdf_bytes: 2 * GIB,
            max_render_side: 10_000,
            archive_max_entries: 200_000,
            archive_max_total_bytes: 50 * GIB,
            archive_max_ratio: 500,
            archive_ratio_grace_bytes: 256 * MIB,
            max_import_files: 10_000,
            max_import_depth: 32,
            media_min_timeout: Duration::from_secs(30 * 60),
            media_timeout_factor: 20,
            media_max_timeout: Duration::from_secs(12 * 60 * 60),
        }
    }
}

impl Limits {
    pub fn image_limits(&self) -> image::Limits {
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(self.max_image_side);
        limits.max_image_height = Some(self.max_image_side);
        limits.max_alloc = Some(self.max_image_alloc_bytes);
        limits
    }

    /// Time budget for converting media of the given duration.
    pub fn media_timeout(&self, duration: Option<Duration>) -> Duration {
        let scaled = duration
            .map(|d| d * self.media_timeout_factor)
            .unwrap_or(self.media_min_timeout);
        scaled.clamp(self.media_min_timeout, self.media_max_timeout)
    }
}
