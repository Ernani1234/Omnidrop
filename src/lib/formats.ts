import type { MessageKey } from "../i18n";
import type { FileFormat } from "./bindings/FileFormat";
import type { FormatFamily } from "./bindings/FormatFamily";
import type { Preset } from "./bindings/Preset";

export interface FormatInfo {
  label: string;
  /** Short plain-language description, shown when the format is chosen. */
  hint?: MessageKey;
  /** Quality presets make sense (lossy encoders). */
  presets: "quality" | "archive" | "none";
}

export const FORMATS: Record<FileFormat, FormatInfo> = {
  png: { label: "PNG", hint: "fmt.png", presets: "none" },
  jpeg: { label: "JPG", hint: "fmt.jpeg", presets: "quality" },
  webp: { label: "WebP", hint: "fmt.webp", presets: "quality" },
  avif: { label: "AVIF", hint: "fmt.avif", presets: "quality" },
  tiff: { label: "TIFF", hint: "fmt.tiff", presets: "none" },
  bmp: { label: "BMP", hint: "fmt.bmp", presets: "none" },
  gif: { label: "GIF", presets: "none" },
  ico: { label: "ICO", presets: "none" },
  pdf: { label: "PDF", presets: "none" },
  mp3: { label: "MP3", hint: "fmt.mp3", presets: "quality" },
  wav: { label: "WAV", hint: "fmt.wav", presets: "none" },
  flac: { label: "FLAC", hint: "fmt.flac", presets: "none" },
  aac: { label: "AAC", presets: "none" },
  m4a: { label: "AAC", hint: "fmt.m4a", presets: "quality" },
  ogg: { label: "OGG", presets: "none" },
  mp4: { label: "MP4", hint: "fmt.mp4", presets: "quality" },
  mov: { label: "MOV", presets: "none" },
  mkv: { label: "MKV", hint: "fmt.mkv", presets: "quality" },
  webm: { label: "WebM", hint: "fmt.webm", presets: "quality" },
  avi: { label: "AVI", presets: "none" },
  zip: { label: "ZIP", hint: "fmt.zip", presets: "archive" },
  tar: { label: "TAR", hint: "fmt.tar", presets: "none" },
  targz: { label: "TAR.GZ", hint: "fmt.targz", presets: "archive" },
  "7z": { label: "7z", hint: "fmt.7z", presets: "archive" },
  document: { label: "DOC", presets: "none" },
};

export const QUALITY_PRESETS: Preset[] = ["smallest", "balanced", "quality"];
export const ARCHIVE_PRESETS: Preset[] = ["smallest", "balanced", "fastest"];

/** Preferred display order for archive outputs (most compatible first). */
export const ARCHIVE_ORDER: FileFormat[] = ["zip", "7z", "targz", "tar"];

export const FAMILY_LABEL: Record<FormatFamily, MessageKey> = {
  image: "family.image",
  pdf: "family.pdf",
  audio: "family.audio",
  video: "family.video",
  archive: "family.archive",
  document: "family.document",
};

export function formatLabel(format: FileFormat | null | undefined): string {
  return format ? FORMATS[format].label : "?";
}
