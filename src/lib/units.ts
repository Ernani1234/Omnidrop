// Locale-aware formatting of sizes, durations and dimensions.

import type { Locale } from "../i18n";
import type { FileDetails } from "./bindings/FileDetails";

const UNITS = ["B", "KB", "MB", "GB", "TB"];

export function formatBytes(bytes: number, locale: Locale): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return `0 B`;
  const exponent = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), UNITS.length - 1);
  const value = bytes / 1024 ** exponent;
  const digits = exponent === 0 ? 0 : value < 10 ? 1 : 0;
  const number = new Intl.NumberFormat(locale, { maximumFractionDigits: digits, minimumFractionDigits: digits }).format(value);
  return `${number} ${UNITS[exponent]}`;
}

export function formatDuration(ms: number): string {
  const total = Math.round(ms / 1000);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return hours > 0 ? `${hours}:${pad(minutes)}:${pad(seconds)}` : `${minutes}:${pad(seconds)}`;
}

export function formatNumber(value: number, locale: Locale): string {
  return new Intl.NumberFormat(locale).format(value);
}

/** Short technical description of a file, e.g. "1920 × 1080". */
export function describeDetails(
  details: FileDetails | null,
  locale: Locale,
  t: (key: "queue.pages" | "queue.pagesOne" | "queue.entries", params?: Record<string, string | number>) => string,
): string | null {
  if (!details) return null;
  switch (details.kind) {
    case "image":
      return `${formatNumber(details.width, locale)} × ${formatNumber(details.height, locale)}`;
    case "pdf":
      return details.pages === 1 ? t("queue.pagesOne") : t("queue.pages", { count: formatNumber(details.pages, locale) });
    case "media": {
      const parts: string[] = [];
      if (details.durationMs != null) parts.push(formatDuration(details.durationMs));
      if (details.width && details.height) parts.push(`${details.width} × ${details.height}`);
      return parts.join(" · ") || null;
    }
    case "archive":
      return t("queue.entries", { count: formatNumber(details.entries, locale) });
  }
}

export function percent(value: number): number {
  return Math.round(Math.min(1, Math.max(0, value)) * 100);
}
