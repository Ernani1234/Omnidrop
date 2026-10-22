import {
  Archive,
  FileAudio,
  FileImage,
  FileStack,
  FileVideo,
  Images,
  type LucideIcon,
  PackageOpen,
  RotateCw,
  Scissors,
} from "lucide-react";
import type { MessageKey } from "../i18n";
import type { ConversionCapability } from "./bindings/ConversionCapability";
import type { FileFormat } from "./bindings/FileFormat";
import type { FileInfo } from "./bindings/FileInfo";
import type { FormatFamily } from "./bindings/FormatFamily";
import type { OperationId } from "./bindings/OperationId";
import type { OperationRequest } from "./bindings/OperationRequest";
import type { Preset } from "./bindings/Preset";

export interface OperationMeta {
  id: OperationId;
  family: FormatFamily;
  icon: LucideIcon;
  title: MessageKey;
  description: MessageKey;
  verb: MessageKey;
  running: MessageKey;
}

const meta = (id: OperationId, family: FormatFamily, icon: LucideIcon): OperationMeta => ({
  id,
  family,
  icon,
  title: `op.${id}` as MessageKey,
  description: `op.${id}.desc` as MessageKey,
  verb: `verb.${id}` as MessageKey,
  running: `running.${id}` as MessageKey,
});

/** Display order inside a family; archive creation always comes last. */
export const OPERATIONS: OperationMeta[] = [
  meta("imageConvert", "image", FileImage),
  meta("pdfMerge", "pdf", FileStack),
  meta("pdfSplit", "pdf", Scissors),
  meta("pdfRotate", "pdf", RotateCw),
  meta("pdfToImages", "pdf", Images),
  meta("audioConvert", "audio", FileAudio),
  meta("videoConvert", "video", FileVideo),
  meta("archiveExtract", "archive", PackageOpen),
  meta("archiveCreate", "archive", Archive),
];

export const OPERATION_BY_ID = Object.fromEntries(OPERATIONS.map((m) => [m.id, m])) as Record<OperationId, OperationMeta>;

export type ResizeMode = "none" | "fit" | "percent";
export type PageMode = "all" | "some";

export interface OperationOptions {
  imageConvert: {
    target: FileFormat;
    preset: Preset;
    customQuality: boolean;
    quality: number;
    lossless: boolean;
    resizeMode: ResizeMode;
    maxWidth: string;
    maxHeight: string;
    percent: number;
    background: string;
  };
  pdfMerge: { outputName: string };
  pdfSplit: { mode: "each" | "ranges"; ranges: string };
  pdfRotate: { angle: 90 | 180 | 270; mode: PageMode; pages: string };
  pdfToImages: { target: FileFormat; preset: Preset; mode: PageMode; pages: string };
  audioConvert: { target: FileFormat; preset: Preset };
  videoConvert: { target: FileFormat; preset: Preset; maxHeight: number | null; removeAudio: boolean };
  archiveCreate: { target: FileFormat; preset: Preset; outputName: string };
  archiveExtract: Record<string, never>;
}

export const DEFAULT_OPTIONS: OperationOptions = {
  imageConvert: {
    target: "webp",
    preset: "balanced",
    customQuality: false,
    quality: 80,
    lossless: false,
    resizeMode: "none",
    maxWidth: "1920",
    maxHeight: "1920",
    percent: 50,
    background: "#ffffff",
  },
  pdfMerge: { outputName: "" },
  pdfSplit: { mode: "each", ranges: "" },
  pdfRotate: { angle: 90, mode: "all", pages: "" },
  pdfToImages: { target: "png", preset: "balanced", mode: "all", pages: "" },
  audioConvert: { target: "mp3", preset: "balanced" },
  videoConvert: { target: "mp4", preset: "balanced", maxHeight: null, removeAudio: false },
  archiveCreate: { target: "zip", preset: "balanced", outputName: "" },
  archiveExtract: {},
};

export interface RequestDefaults {
  mergeName: string;
  archiveName?: string;
}

const positive = (raw: string): number | null => {
  const value = Number.parseInt(raw, 10);
  return Number.isFinite(value) && value > 0 ? value : null;
};

const nonEmpty = (raw: string): string | null => (raw.trim() ? raw.trim() : null);

/** Turns the form state into the request the planner understands. */
export function buildRequest(op: OperationId, options: OperationOptions, defaults: RequestDefaults): OperationRequest {
  switch (op) {
    case "imageConvert": {
      const o = options.imageConvert;
      const resize =
        o.resizeMode === "fit" && (positive(o.maxWidth) || positive(o.maxHeight))
          ? { mode: "fit" as const, maxWidth: positive(o.maxWidth), maxHeight: positive(o.maxHeight) }
          : o.resizeMode === "percent"
            ? { mode: "percent" as const, percent: Math.round(o.percent) }
            : null;
      return {
        op,
        target: o.target,
        preset: o.preset,
        quality: o.customQuality ? Math.round(o.quality) : null,
        lossless: o.target === "webp" && o.lossless,
        resize,
        background: o.background,
      };
    }
    case "pdfMerge":
      return { op, outputName: nonEmpty(options.pdfMerge.outputName) ?? defaults.mergeName };
    case "pdfSplit":
      return { op, ranges: options.pdfSplit.mode === "ranges" ? options.pdfSplit.ranges.trim() || null : null };
    case "pdfRotate": {
      const o = options.pdfRotate;
      return { op, angle: o.angle, pages: o.mode === "some" ? nonEmpty(o.pages) : null };
    }
    case "pdfToImages": {
      const o = options.pdfToImages;
      return { op, target: o.target, preset: o.preset, pages: o.mode === "some" ? nonEmpty(o.pages) : null };
    }
    case "audioConvert":
      return { op, ...options.audioConvert };
    case "videoConvert":
      return { op, ...options.videoConvert };
    case "archiveCreate": {
      const o = options.archiveCreate;
      return { op, target: o.target, preset: o.preset, outputName: nonEmpty(o.outputName) ?? defaults.archiveName ?? null };
    }
    case "archiveExtract":
      return { op };
  }
}

export function accepts(capability: ConversionCapability, file: FileInfo): boolean {
  if (file.issue) return false;
  if (capability.acceptsAnyInput) return true;
  return file.format != null && capability.inputFormats.includes(file.format);
}

export interface OperationChoice {
  meta: OperationMeta;
  capability: ConversionCapability;
  count: number;
}

/**
 * Operations that apply to at least one file, most relevant first: the
 * family with the most files leads, and "Compress" is always last.
 */
export function operationsFor(files: FileInfo[], capabilities: ConversionCapability[]): OperationChoice[] {
  const familyCount = new Map<FormatFamily, number>();
  for (const file of files) {
    if (file.family) familyCount.set(file.family, (familyCount.get(file.family) ?? 0) + 1);
  }
  const choices: OperationChoice[] = [];
  OPERATIONS.forEach((m) => {
    const capability = capabilities.find((c) => c.id === m.id);
    if (!capability) return;
    const count = files.filter((f) => accepts(capability, f)).length;
    const minimum = capability.inputMode.kind === "combine" ? capability.inputMode.minFiles : 1;
    if (count >= minimum) choices.push({ meta: m, capability, count });
  });
  const rank = (choice: OperationChoice) => {
    if (choice.meta.id === "archiveCreate") return -1;
    // Video files are also accepted by audio conversion; rank by own family.
    return familyCount.get(choice.meta.family) ?? 0;
  };
  return choices
    .map((choice, index) => ({ choice, index }))
    .sort((a, b) => rank(b.choice) - rank(a.choice) || a.index - b.index)
    .map(({ choice }) => choice);
}

/** The operation to preselect: the first one that can run. */
export function defaultOperation(choices: OperationChoice[]): OperationId | null {
  return choices.find((c) => c.capability.available)?.meta.id ?? choices[0]?.meta.id ?? null;
}

/** Name suggested for a new archive: the file's own name, the imported folder, or `fallback`. */
export function archiveDefaultName(files: FileInfo[], fallback: string): string {
  if (files.length === 1) return files[0].name.replace(/\.[^.]+$/, "") || fallback;
  const roots = new Set(files.map((f) => f.relativeDir?.split("/")[0] ?? ""));
  const [root] = [...roots];
  return roots.size === 1 && root ? root : fallback;
}

/** Keeps targets valid for this computer (for example, when an encoder is missing). */
export function withAvailableTargets(
  options: OperationOptions,
  capabilities: ConversionCapability[],
): OperationOptions {
  const fix = <K extends "audioConvert" | "videoConvert" | "imageConvert" | "pdfToImages" | "archiveCreate">(op: K) => {
    const capability = capabilities.find((c) => c.id === op);
    const current = options[op] as { target: FileFormat };
    if (!capability || capability.outputFormats.length === 0 || capability.outputFormats.includes(current.target)) {
      return options[op];
    }
    return { ...options[op], target: capability.outputFormats[0] };
  };
  return {
    ...options,
    imageConvert: fix("imageConvert"),
    pdfToImages: fix("pdfToImages"),
    audioConvert: fix("audioConvert"),
    videoConvert: fix("videoConvert"),
    archiveCreate: fix("archiveCreate"),
  };
}
