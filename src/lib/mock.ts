// Simulated backend for running the UI in a plain browser during
// development (`npm run dev` without Tauri). Loaded only in dev builds.

import type { Backend } from "./api";
import type { Bootstrap } from "./bindings/Bootstrap";
import type { ConversionCapability } from "./bindings/ConversionCapability";
import type { FileFormat } from "./bindings/FileFormat";
import type { FileInfo } from "./bindings/FileInfo";
import type { JobEvent } from "./bindings/JobEvent";
import type { OperationRequest } from "./bindings/OperationRequest";
import type { Plan } from "./bindings/Plan";
import type { Settings } from "./bindings/Settings";
import { accepts } from "./operations";

const caps = (): ConversionCapability[] => {
  const base = {
    unavailableOutputFormats: [],
    supportsBatch: true,
    supportsQualitySettings: true,
    cancellation: "betweenSteps" as const,
    requiresExternalBinary: null,
    available: true,
    unavailableReason: null,
    presets: ["smallest", "balanced", "quality"] as const,
    maxInputBytes: 2 * 1024 ** 3,
    acceptsAnyInput: false,
    inputMode: { kind: "perFile" as const },
  };
  const make = (c: Partial<ConversionCapability> & Pick<ConversionCapability, "id" | "family" | "inputFormats" | "outputFormats">): ConversionCapability => ({
    ...base,
    presets: [...base.presets],
    ...c,
  });
  return [
    make({ id: "imageConvert", family: "image", inputFormats: ["png", "jpeg", "webp", "tiff", "bmp", "gif", "ico"], outputFormats: ["png", "jpeg", "webp", "avif", "tiff", "bmp"] }),
    make({ id: "pdfMerge", family: "pdf", inputFormats: ["pdf"], outputFormats: [], inputMode: { kind: "combine", minFiles: 2 } }),
    make({ id: "pdfSplit", family: "pdf", inputFormats: ["pdf"], outputFormats: [] }),
    make({ id: "pdfRotate", family: "pdf", inputFormats: ["pdf"], outputFormats: [] }),
    make({ id: "pdfToImages", family: "pdf", inputFormats: ["pdf"], outputFormats: ["png", "jpeg", "webp"] }),
    make({ id: "audioConvert", family: "audio", inputFormats: ["mp3", "wav", "flac", "m4a", "mp4", "mkv"], outputFormats: ["mp3", "m4a", "flac", "wav"], requiresExternalBinary: "ffmpeg" }),
    make({ id: "videoConvert", family: "video", inputFormats: ["mp4", "mov", "mkv", "webm", "avi"], outputFormats: ["mp4", "webm", "mkv"], requiresExternalBinary: "ffmpeg" }),
    make({ id: "archiveCreate", family: "archive", inputFormats: [], outputFormats: ["zip", "tar", "targz", "7z"], acceptsAnyInput: true, inputMode: { kind: "combine", minFiles: 1 }, presets: ["smallest", "balanced", "fastest"] }),
    make({ id: "archiveExtract", family: "archive", inputFormats: ["zip", "tar", "targz", "7z"], outputFormats: [], presets: [] }),
  ];
};

let counter = 0;
const file = (name: string, format: FileFormat | null, size: number, details: FileInfo["details"], extra: Partial<FileInfo> = {}): FileInfo => ({
  id: `mock-${++counter}`,
  name,
  displayPath: `C:\\Users\\demo\\Pictures\\${name}`,
  relativeDir: null,
  size,
  format,
  family: format ? (["png", "jpeg", "webp", "tiff", "bmp", "gif"].includes(format) ? "image" : format === "pdf" ? "pdf" : "archive") : null,
  extension: name.split(".").pop() ?? null,
  extensionMismatch: false,
  details,
  issue: null,
  ...extra,
});

function sampleFiles(): FileInfo[] {
  return [
    file("praia-ao-entardecer.png", "png", 4_812_331, { kind: "image", width: 3024, height: 2016, hasAlpha: false, animated: false }),
    file("logo-transparente.png", "png", 182_004, { kind: "image", width: 1200, height: 1200, hasAlpha: true, animated: false }),
    file("cardapio-2026.jpg", "png", 905_112, { kind: "image", width: 1600, height: 2400, hasAlpha: false, animated: false }, { extensionMismatch: true }),
    file("relatorio-anual.pdf", "pdf", 2_220_001, { kind: "pdf", pages: 18 }),
    file("notas.txt", null, 1_024, null),
  ];
}

const params = new URLSearchParams(globalThis.location?.search ?? "");

export function createMockBackend(): Backend {
  const listeners = new Map<string, Set<(payload: unknown) => void>>();
  const emit = (event: string, payload: unknown) => listeners.get(event)?.forEach((h) => h(payload));
  let files: FileInfo[] = params.get("demo") ? sampleFiles() : [];
  let settings: Settings = {
    theme: (params.get("theme") as Settings["theme"]) ?? "system",
    language: (params.get("lang") as Settings["language"]) ?? "auto",
    outputMode: "nextToSource",
    outputFolder: null,
    collision: "rename",
    ffmpegPath: null,
  };
  const tickMs = params.get("demo") === "running" ? 4000 : 120;
  const capabilities = caps();
  const cancelled = new Set<string>();

  const plan = (request: OperationRequest, ids?: string[] | null): Plan => {
    const capability = capabilities.find((c) => c.id === request.op)!;
    const selected = ids ? files.filter((f) => ids.includes(f.id)) : files;
    const ok = selected.filter((f) => accepts(capability, f));
    const skipped = selected
      .filter((f) => !ok.includes(f))
      .map((f) => ({ fileId: f.id, name: f.name, error: { code: f.format ? ("unsupportedConversion" as const) : ("unsupportedFormat" as const), detail: "" } }));
    const combine = capability.inputMode.kind === "combine";
    const tasks = (combine ? [ok] : ok.map((f) => [f])).filter((group) => group.length > 0).map((group, i) => ({
      id: `task-${Date.now()}-${i}`,
      fileIds: group.map((f) => f.id),
      label: group[0].name,
      outputName: group[0].name,
      notes: request.op === "imageConvert" && request.target === "jpeg" && group[0].details?.kind === "image" && group[0].details.hasAlpha ? [{ code: "transparencyFlattened" as const }] : [],
    }));
    return { jobId: `job-${Date.now()}`, operation: request.op, request, outputSettings: { location: { kind: "nextToSource" }, collision: settings.collision }, tasks, skipped, expectedOutputs: tasks.length };
  };

  const runJob = (p: Plan) => {
    emit("omnidrop://job", { type: "jobStarted", jobId: p.jobId, total: p.tasks.length } satisfies JobEvent);
    let index = 0;
    const outputs: Array<Extract<JobEvent, { type: "taskFinished" }>["outcome"]> = [];
    const next = () => {
      const task = p.tasks[index++];
      if (!task) {
        const done = outputs.filter((o) => o.status === "succeeded");
        emit("omnidrop://job", {
          type: "jobFinished",
          jobId: p.jobId,
          summary: {
            succeeded: done.length,
            failed: outputs.filter((o) => o.status === "failed").length,
            skipped: 0,
            cancelled: outputs.filter((o) => o.status === "cancelled").length,
            outputs: done.flatMap((o) => (o.status === "succeeded" ? o.outputs : [])),
            durationMs: 1200,
            inputBytes: 9_000_000,
            outputBytes: 2_100_000,
          },
        } satisfies JobEvent);
        return;
      }
      if (cancelled.has(p.jobId)) {
        const outcome = { status: "cancelled" as const };
        outputs.push(outcome);
        emit("omnidrop://job", { type: "taskFinished", jobId: p.jobId, taskId: task.id, outcome });
        setTimeout(next, 10);
        return;
      }
      emit("omnidrop://job", { type: "taskStarted", jobId: p.jobId, taskId: task.id });
      let progress = 0;
      const tick = setInterval(() => {
        progress += 0.25;
        emit("omnidrop://job", { type: "taskProgress", jobId: p.jobId, taskId: task.id, progress: Math.min(progress, 1) });
        if (progress >= 1) {
          clearInterval(tick);
          const outcome = {
            status: "succeeded" as const,
            outputs: [{ id: `out-${task.id}`, name: task.outputName.replace(/\.\w+$/, ".webp"), displayPath: task.outputName, isDir: false, size: 420_000, renamed: false }],
            notes: task.notes,
          };
          outputs.push(outcome);
          emit("omnidrop://job", { type: "taskFinished", jobId: p.jobId, taskId: task.id, outcome });
          next();
        }
      }, tickMs);
    };
    setTimeout(next, 50);
  };

  const handlers: Record<string, (args: Record<string, unknown>) => unknown> = {
    bootstrap: (): Bootstrap => ({
      version: "0.1.0-dev",
      platform: "windows",
      settings,
      files,
      tools: { ready: true, tools: { ffmpeg: null, ffprobe: null }, capabilities },
      history: [],
    }),
    pick_files: () => {
      const added = sampleFiles();
      files = [...files, ...added];
      setTimeout(() => emit("omnidrop://files", { files: added, truncated: false }), 50);
    },
    pick_folder: () => handlers.pick_files({}),
    remove_files: ({ ids }) => {
      files = files.filter((f) => !(ids as string[]).includes(f.id));
    },
    clear_files: () => {
      files = [];
    },
    move_file: () => undefined,
    thumbnail: () => null,
    preview_plan: ({ request, fileIds }) => plan(request as OperationRequest, fileIds as string[] | null),
    start_job: ({ request, fileIds }) => {
      const p = plan(request as OperationRequest, fileIds as string[] | null);
      setTimeout(() => runJob(p), 0);
      return p;
    },
    cancel_job: ({ jobId }) => {
      cancelled.add(jobId as string);
    },
    update_settings: ({ patch }) => {
      settings = { ...settings, ...(patch as Partial<Settings>) };
      return settings;
    },
    pick_output_folder: () => settings,
  };

  return {
    async invoke<T>(command: string, args: Record<string, unknown> = {}) {
      const handler = handlers[command];
      return (handler ? handler(args) : undefined) as T;
    },
    async listen(event, handler) {
      const set = listeners.get(event) ?? new Set();
      set.add(handler as (payload: unknown) => void);
      listeners.set(event, set);
      return () => set.delete(handler as (payload: unknown) => void);
    },
  };
}

/** Dev-only: drives the UI into a given state for visual checks (?demo=running|done). */
export async function runDemo(start: (request: OperationRequest) => Promise<void>, openDialog: (name: "settings" | "results" | "history") => void) {
  const demo = params.get("demo");
  const dialog = params.get("dialog");
  if (dialog === "settings" || dialog === "history") setTimeout(() => openDialog(dialog), 400);
  if (dialog === "results") setTimeout(() => openDialog(dialog), 2500);
  if (demo === "running" || demo === "done") {
    await new Promise((r) => setTimeout(r, 300));
    await start({ op: "imageConvert", target: "webp", preset: "balanced", quality: null, lossless: false, resize: null, background: "#ffffff" });
  }
}
