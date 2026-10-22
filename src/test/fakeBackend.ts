import type { Backend } from "../lib/api";
import type { Bootstrap } from "../lib/bindings/Bootstrap";
import type { ConversionCapability } from "../lib/bindings/ConversionCapability";
import type { FileInfo } from "../lib/bindings/FileInfo";
import type { OperationRequest } from "../lib/bindings/OperationRequest";
import type { Plan } from "../lib/bindings/Plan";
import { accepts } from "../lib/operations";

export function capability(partial: Partial<ConversionCapability> & Pick<ConversionCapability, "id" | "family">): ConversionCapability {
  return {
    inputFormats: [],
    acceptsAnyInput: false,
    outputFormats: [],
    unavailableOutputFormats: [],
    inputMode: { kind: "perFile" },
    supportsBatch: true,
    supportsQualitySettings: true,
    cancellation: "betweenSteps",
    requiresExternalBinary: null,
    available: true,
    unavailableReason: null,
    presets: ["smallest", "balanced", "quality"],
    maxInputBytes: 1e12,
    ...partial,
  };
}

export const CAPABILITIES: ConversionCapability[] = [
  capability({ id: "imageConvert", family: "image", inputFormats: ["png", "jpeg", "webp"], outputFormats: ["png", "jpeg", "webp", "avif", "tiff", "bmp"] }),
  capability({ id: "pdfMerge", family: "pdf", inputFormats: ["pdf"], inputMode: { kind: "combine", minFiles: 2 } }),
  capability({ id: "pdfSplit", family: "pdf", inputFormats: ["pdf"] }),
  capability({
    id: "audioConvert",
    family: "audio",
    inputFormats: ["mp3", "wav"],
    available: false,
    unavailableReason: "toolMissing",
    requiresExternalBinary: "ffmpeg",
  }),
  capability({ id: "archiveCreate", family: "archive", acceptsAnyInput: true, outputFormats: ["zip", "7z", "tar", "targz"], inputMode: { kind: "combine", minFiles: 1 } }),
];

let n = 0;
export function imageFile(name: string, extra: Partial<FileInfo> = {}): FileInfo {
  n += 1;
  return {
    id: `f${n}`,
    name,
    displayPath: `/photos/${name}`,
    relativeDir: null,
    size: 1000,
    format: "png",
    family: "image",
    extension: "png",
    extensionMismatch: false,
    details: { kind: "image", width: 10, height: 10, hasAlpha: false, animated: false },
    issue: null,
    ...extra,
  };
}

export interface FakeBackend extends Backend {
  calls: Array<{ command: string; args?: Record<string, unknown> }>;
  emit: (event: string, payload: unknown) => void;
  files: FileInfo[];
}

export function createFakeBackend(initial: Partial<Bootstrap> = {}): FakeBackend {
  const listeners = new Map<string, Set<(payload: unknown) => void>>();
  const calls: FakeBackend["calls"] = [];
  const backend: FakeBackend = {
    calls,
    files: [...(initial.files ?? [])],
    emit: (event, payload) => listeners.get(event)?.forEach((handler) => handler(payload)),
    async listen(event, handler) {
      const set = listeners.get(event) ?? new Set();
      set.add(handler as (payload: unknown) => void);
      listeners.set(event, set);
      return () => set.delete(handler as (payload: unknown) => void);
    },
    async invoke<T>(command: string, args?: Record<string, unknown>) {
      calls.push({ command, args });
      switch (command) {
        case "bootstrap":
          return {
            version: "0.1.0",
            platform: "linux",
            settings: { theme: "light", language: "pt-BR", outputMode: "nextToSource", outputFolder: null, collision: "rename", ffmpegPath: null },
            files: backend.files,
            tools: { ready: true, tools: { ffmpeg: null, ffprobe: null }, capabilities: CAPABILITIES },
            history: [],
            ...initial,
          } as T;
        case "preview_plan":
        case "start_job":
          return planFor(backend.files, args!.request as OperationRequest) as T;
        default:
          return undefined as T;
      }
    },
  };
  return backend;
}

export function planFor(files: FileInfo[], request: OperationRequest): Plan {
  const cap = CAPABILITIES.find((c) => c.id === request.op)!;
  const ok = files.filter((f) => accepts(cap, f));
  const groups = cap.inputMode.kind === "combine" ? [ok] : ok.map((f) => [f]);
  return {
    jobId: "job-1",
    operation: request.op,
    request,
    outputSettings: { location: { kind: "nextToSource" }, collision: "rename" },
    tasks: groups.map((group, i) => ({ id: `t${i}`, fileIds: group.map((f) => f.id), label: group[0].name, outputName: group[0].name, notes: [] })),
    skipped: files
      .filter((f) => !ok.includes(f))
      .map((f) => ({ fileId: f.id, name: f.name, error: { code: "unsupportedConversion", detail: "" } })),
    expectedOutputs: groups.length,
  };
}
