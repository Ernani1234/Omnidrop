//! File formats known to OmniDrop and content-based detection.
//!
//! Detection always prefers the file's real signature over its extension, so a
//! PNG renamed to `.jpg` is still treated as a PNG.

use serde::{Deserialize, Serialize};
use std::path::Path;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum FileFormat {
    Png,
    Jpeg,
    Webp,
    Avif,
    Tiff,
    Bmp,
    Gif,
    Ico,
    Pdf,
    Mp3,
    Wav,
    Flac,
    Aac,
    M4a,
    Ogg,
    Mp4,
    Mov,
    Mkv,
    Webm,
    Avi,
    Zip,
    Tar,
    #[serde(rename = "targz")]
    TarGz,
    #[serde(rename = "7z")]
    SevenZ,
    /// Editable office documents (DOCX, ODT, XLS, ...). Recognized so the UI can
    /// explain that converting them is not offered yet.
    Document,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FormatFamily {
    Image,
    Pdf,
    Audio,
    Video,
    Archive,
    Document,
}

const OFFICE_EXTENSIONS: &[&str] = &[
    "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "rtf", "epub", "pages",
    "key", "numbers",
];

impl FileFormat {
    pub const ALL: [FileFormat; 25] = [
        FileFormat::Png,
        FileFormat::Jpeg,
        FileFormat::Webp,
        FileFormat::Avif,
        FileFormat::Tiff,
        FileFormat::Bmp,
        FileFormat::Gif,
        FileFormat::Ico,
        FileFormat::Pdf,
        FileFormat::Mp3,
        FileFormat::Wav,
        FileFormat::Flac,
        FileFormat::Aac,
        FileFormat::M4a,
        FileFormat::Ogg,
        FileFormat::Mp4,
        FileFormat::Mov,
        FileFormat::Mkv,
        FileFormat::Webm,
        FileFormat::Avi,
        FileFormat::Zip,
        FileFormat::Tar,
        FileFormat::TarGz,
        FileFormat::SevenZ,
        FileFormat::Document,
    ];

    pub fn family(self) -> FormatFamily {
        use FileFormat::*;
        match self {
            Png | Jpeg | Webp | Avif | Tiff | Bmp | Gif | Ico => FormatFamily::Image,
            Pdf => FormatFamily::Pdf,
            Mp3 | Wav | Flac | Aac | M4a | Ogg => FormatFamily::Audio,
            Mp4 | Mov | Mkv | Webm | Avi => FormatFamily::Video,
            Zip | Tar | TarGz | SevenZ => FormatFamily::Archive,
            Document => FormatFamily::Document,
        }
    }

    /// Canonical extension used when writing this format.
    pub fn extension(self) -> &'static str {
        self.extensions()[0]
    }

    /// All extensions that usually carry this format, canonical first.
    pub fn extensions(self) -> &'static [&'static str] {
        use FileFormat::*;
        match self {
            Png => &["png", "apng"],
            Jpeg => &["jpg", "jpeg", "jpe", "jfif"],
            Webp => &["webp"],
            Avif => &["avif"],
            Tiff => &["tiff", "tif"],
            Bmp => &["bmp", "dib"],
            Gif => &["gif"],
            Ico => &["ico"],
            Pdf => &["pdf"],
            Mp3 => &["mp3"],
            Wav => &["wav", "wave"],
            Flac => &["flac"],
            Aac => &["aac"],
            M4a => &["m4a", "m4b"],
            Ogg => &["ogg", "oga", "opus"],
            Mp4 => &["mp4", "m4v"],
            Mov => &["mov", "qt"],
            Mkv => &["mkv"],
            Webm => &["webm"],
            Avi => &["avi"],
            Zip => &["zip"],
            Tar => &["tar"],
            TarGz => &["tar.gz", "tgz"],
            SevenZ => &["7z"],
            Document => OFFICE_EXTENSIONS,
        }
    }

    pub fn from_extension(ext: &str) -> Option<Self> {
        let ext = ext.to_ascii_lowercase();
        Self::ALL
            .into_iter()
            .find(|f| f.extensions().contains(&ext.as_str()))
    }

    /// Whether the format can store an alpha channel.
    pub fn supports_alpha(self) -> bool {
        use FileFormat::*;
        matches!(self, Png | Webp | Avif | Tiff | Bmp | Gif | Ico)
    }

    /// Whether `ext` is a usual extension for this format. Used to flag files
    /// whose extension disagrees with their content.
    pub fn matches_extension(self, ext: &str) -> bool {
        let ext = ext.to_ascii_lowercase();
        if self.extensions().contains(&ext.as_str()) {
            return true;
        }
        // Containers that legitimately share a signature.
        matches!(
            (self, ext.as_str()),
            (FileFormat::Mp4, "m4a" | "m4b" | "mov" | "3gp")
                | (FileFormat::M4a, "mp4")
                | (FileFormat::Mov, "mp4")
                | (FileFormat::Mkv, "mka" | "mk3d")
                | (FileFormat::Ogg, "ogv")
        )
    }
}

/// Lowercased extension, treating `.tar.gz` as a single extension.
pub fn extension_of(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    if name.ends_with(".tar.gz") {
        return Some("tar.gz".to_string());
    }
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
}

/// Number of leading bytes [`sniff`] needs to see.
pub const SNIFF_LEN: usize = 4096;

/// Identifies a format from the first bytes of a file.
///
/// Returns `None` for unknown content. Gzip streams are not resolved here
/// because telling `.tar.gz` apart requires decompressing; see
/// [`is_gzip`] and the inspector.
pub fn sniff(header: &[u8]) -> Option<FileFormat> {
    use FileFormat::*;
    let h = header;
    let starts = |sig: &[u8]| h.starts_with(sig);

    if starts(b"\x89PNG\r\n\x1a\n") {
        return Some(Png);
    }
    if starts(&[0xFF, 0xD8, 0xFF]) {
        return Some(Jpeg);
    }
    if starts(b"GIF87a") || starts(b"GIF89a") {
        return Some(Gif);
    }
    if h.len() >= 12 && &h[0..4] == b"RIFF" {
        match &h[8..12] {
            b"WEBP" => return Some(Webp),
            b"WAVE" => return Some(Wav),
            b"AVI " => return Some(Avi),
            _ => {}
        }
    }
    if starts(b"II*\0") || starts(b"MM\0*") {
        return Some(Tiff);
    }
    if h.len() >= 26 && starts(b"BM") {
        return Some(Bmp);
    }
    if let Some(format) = sniff_iso_bmff(h) {
        return Some(format);
    }
    if h.len() >= 6 && starts(&[0, 0, 1, 0]) && (h[4] != 0 || h[5] != 0) {
        return Some(Ico);
    }
    if find(&h[..h.len().min(1024)], b"%PDF-").is_some() {
        return Some(Pdf);
    }
    if starts(b"fLaC") {
        return Some(Flac);
    }
    if starts(b"OggS") {
        return Some(Ogg);
    }
    if starts(b"ID3") {
        return Some(Mp3);
    }
    if starts(&[0x1A, 0x45, 0xDF, 0xA3]) {
        let window = &h[..h.len().min(64)];
        return Some(if find(window, b"webm").is_some() {
            Webm
        } else {
            Mkv
        });
    }
    if starts(b"PK\x03\x04") || starts(b"PK\x05\x06") || starts(b"PK\x07\x08") {
        return Some(Zip);
    }
    if starts(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]) {
        return Some(SevenZ);
    }
    if h.len() >= 262 && &h[257..262] == b"ustar" {
        return Some(Tar);
    }
    if starts(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) {
        return Some(Document);
    }
    if h.len() >= 2 && h[0] == 0xFF {
        // ADTS AAC: sync word 0xFFF with layer bits 00.
        if h[1] & 0xF6 == 0xF0 {
            return Some(Aac);
        }
        // MPEG audio frame sync, layer III.
        if h[1] & 0xE0 == 0xE0 && (h[1] >> 1) & 0x03 == 0x01 {
            return Some(Mp3);
        }
    }
    None
}

pub fn is_gzip(header: &[u8]) -> bool {
    header.starts_with(&[0x1F, 0x8B])
}

/// Resolves the final format from content plus extension. Zip and OLE
/// containers with office extensions are reported as documents.
pub fn resolve(sniffed: Option<FileFormat>, ext: Option<&str>) -> Option<FileFormat> {
    match (sniffed, ext) {
        (Some(FileFormat::Zip), Some(ext)) if OFFICE_EXTENSIONS.contains(&ext) => {
            Some(FileFormat::Document)
        }
        (Some(format), _) => Some(format),
        (None, Some("rtf")) => Some(FileFormat::Document),
        (None, _) => None,
    }
}

fn sniff_iso_bmff(h: &[u8]) -> Option<FileFormat> {
    if h.len() < 16 || &h[4..8] != b"ftyp" {
        return None;
    }
    let box_len = u32::from_be_bytes([h[0], h[1], h[2], h[3]]) as usize;
    let end = box_len.clamp(16, h.len());
    let major = &h[8..12];
    let compatible: Vec<&[u8]> = h[16..end]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| c.as_slice())
        .collect();
    let has = |brand: &[u8]| major == brand || compatible.contains(&brand);

    if has(b"avif") || has(b"avis") {
        return Some(FileFormat::Avif);
    }
    if has(b"heic") || has(b"heix") || has(b"mif1") || has(b"msf1") {
        // HEIF/HEIC images are not supported.
        return None;
    }
    match major {
        b"M4A " | b"M4B " | b"M4P " => Some(FileFormat::M4a),
        b"qt  " => Some(FileFormat::Mov),
        _ => Some(FileFormat::Mp4),
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn padded(prefix: &[u8]) -> Vec<u8> {
        let mut v = prefix.to_vec();
        v.resize(600, 0);
        v
    }

    #[test]
    fn sniffs_images() {
        assert_eq!(sniff(&padded(b"\x89PNG\r\n\x1a\n")), Some(FileFormat::Png));
        assert_eq!(
            sniff(&padded(&[0xFF, 0xD8, 0xFF, 0xE0])),
            Some(FileFormat::Jpeg)
        );
        assert_eq!(
            sniff(&padded(b"RIFF\0\0\0\0WEBPVP8 ")),
            Some(FileFormat::Webp)
        );
        assert_eq!(sniff(&padded(b"GIF89a")), Some(FileFormat::Gif));
        assert_eq!(sniff(&padded(b"II*\0")), Some(FileFormat::Tiff));
        assert_eq!(sniff(&padded(b"BM")), Some(FileFormat::Bmp));
    }

    #[test]
    fn sniffs_iso_bmff_brands() {
        let mut avif = vec![0, 0, 0, 0x1C];
        avif.extend_from_slice(b"ftypavif\0\0\0\0avifmif1miaf");
        assert_eq!(sniff(&padded(&avif)), Some(FileFormat::Avif));

        let mut mp4 = vec![0, 0, 0, 0x18];
        mp4.extend_from_slice(b"ftypisom\0\0\x02\0isomiso2");
        assert_eq!(sniff(&padded(&mp4)), Some(FileFormat::Mp4));

        let mut m4a = vec![0, 0, 0, 0x14];
        m4a.extend_from_slice(b"ftypM4A \0\0\0\0M4A ");
        assert_eq!(sniff(&padded(&m4a)), Some(FileFormat::M4a));

        let mut heic = vec![0, 0, 0, 0x18];
        heic.extend_from_slice(b"ftypheic\0\0\0\0mif1heic");
        assert_eq!(sniff(&padded(&heic)), None);
    }

    #[test]
    fn sniffs_documents_media_and_archives() {
        assert_eq!(sniff(&padded(b"%PDF-1.7")), Some(FileFormat::Pdf));
        assert_eq!(sniff(&padded(b"fLaC")), Some(FileFormat::Flac));
        assert_eq!(sniff(&padded(b"ID3\x04")), Some(FileFormat::Mp3));
        assert_eq!(sniff(&padded(&[0xFF, 0xFB, 0x90])), Some(FileFormat::Mp3));
        assert_eq!(sniff(&padded(&[0xFF, 0xF1, 0x50])), Some(FileFormat::Aac));
        assert_eq!(sniff(&padded(b"PK\x03\x04")), Some(FileFormat::Zip));
        assert_eq!(
            sniff(&padded(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C])),
            Some(FileFormat::SevenZ)
        );
        let mut webm = vec![0x1A, 0x45, 0xDF, 0xA3];
        webm.extend_from_slice(b"\x9fB\x86\x81\x01B\xf7\x81\x01B\x82\x84webm");
        assert_eq!(sniff(&padded(&webm)), Some(FileFormat::Webm));
        let mut tar = vec![0u8; 512];
        tar[257..262].copy_from_slice(b"ustar");
        assert_eq!(sniff(&tar), Some(FileFormat::Tar));
    }

    #[test]
    fn unknown_content_is_none() {
        assert_eq!(sniff(b"hello world, this is plain text"), None);
        assert_eq!(sniff(&[]), None);
    }

    #[test]
    fn office_zip_is_document() {
        assert_eq!(
            resolve(Some(FileFormat::Zip), Some("docx")),
            Some(FileFormat::Document)
        );
        assert_eq!(
            resolve(Some(FileFormat::Zip), Some("zip")),
            Some(FileFormat::Zip)
        );
    }

    #[test]
    fn extensions_round_trip() {
        assert_eq!(FileFormat::from_extension("JPG"), Some(FileFormat::Jpeg));
        assert_eq!(FileFormat::from_extension("tgz"), Some(FileFormat::TarGz));
        assert_eq!(
            extension_of(Path::new("a/b/c.TAR.GZ")).as_deref(),
            Some("tar.gz")
        );
        assert!(FileFormat::Jpeg.matches_extension("jpeg"));
        assert!(!FileFormat::Png.matches_extension("jpg"));
        for format in FileFormat::ALL {
            assert!(format.matches_extension(format.extension()));
        }
    }

    #[test]
    fn serde_ids_are_stable() {
        assert_eq!(
            serde_json::to_string(&FileFormat::SevenZ).unwrap(),
            "\"7z\""
        );
        assert_eq!(
            serde_json::to_string(&FileFormat::TarGz).unwrap(),
            "\"targz\""
        );
        assert_eq!(
            serde_json::to_string(&FileFormat::Jpeg).unwrap(),
            "\"jpeg\""
        );
    }
}
