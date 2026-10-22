// Typed access to the Rust backend. Components never call `invoke`
// directly, and the backend can be swapped (tests, browser preview).

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";
import type { Bootstrap } from "./bindings/Bootstrap";
import type { DragEvent } from "./bindings/DragEvent";
import type { ExternalLink } from "./bindings/ExternalLink";
import type { FilesAddedEvent } from "./bindings/FilesAddedEvent";
import type { FormatFamily } from "./bindings/FormatFamily";
import type { InspectingEvent } from "./bindings/InspectingEvent";
import type { JobEvent } from "./bindings/JobEvent";
import type { OperationRequest } from "./bindings/OperationRequest";
import type { Plan } from "./bindings/Plan";
import type { Settings } from "./bindings/Settings";
import type { SettingsPatch } from "./bindings/SettingsPatch";
import type { ToolsPayload } from "./bindings/ToolsPayload";

export interface Backend {
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
  listen<T>(event: string, handler: (payload: T) => void): Promise<() => void>;
}

const tauriBackend: Backend = {
  invoke: (command, args) => tauriInvoke(command, args),
  listen: async (event, handler) => tauriListen(event, (e) => handler(e.payload as never)),
};

let backend: Backend = tauriBackend;

export function setBackend(next: Backend) {
  backend = next;
}

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export const EVENTS = {
  files: "omnidrop://files",
  inspecting: "omnidrop://inspecting",
  drag: "omnidrop://drag",
  job: "omnidrop://job",
  tools: "omnidrop://tools",
} as const;

export interface EventPayloads {
  [EVENTS.files]: FilesAddedEvent;
  [EVENTS.inspecting]: InspectingEvent;
  [EVENTS.drag]: DragEvent;
  [EVENTS.job]: JobEvent;
  [EVENTS.tools]: ToolsPayload;
}

export function on<E extends keyof EventPayloads>(event: E, handler: (payload: EventPayloads[E]) => void) {
  return backend.listen<EventPayloads[E]>(event, handler);
}

const call = <T>(command: string, args?: Record<string, unknown>) => backend.invoke<T>(command, args);

export const api = {
  bootstrap: () => call<Bootstrap>("bootstrap"),
  pickFiles: (family?: FormatFamily) => call<void>("pick_files", { family: family ?? null }),
  pickFolder: () => call<void>("pick_folder"),
  removeFiles: (ids: string[]) => call<void>("remove_files", { ids }),
  clearFiles: () => call<void>("clear_files"),
  moveFile: (id: string, toIndex: number) => call<void>("move_file", { id, toIndex }),
  thumbnail: (id: string) => call<string | null>("thumbnail", { id }),
  previewPlan: (request: OperationRequest, fileIds?: string[]) =>
    call<Plan>("preview_plan", { request, fileIds: fileIds ?? null }),
  startJob: (request: OperationRequest, fileIds?: string[]) =>
    call<Plan>("start_job", { request, fileIds: fileIds ?? null }),
  cancelJob: (jobId: string) => call<void>("cancel_job", { jobId }),
  cancelTask: (jobId: string, taskId: string) => call<void>("cancel_task", { jobId, taskId }),
  openOutput: (outputId: string) => call<void>("open_output", { outputId }),
  revealOutput: (outputId: string) => call<void>("reveal_output", { outputId }),
  openJobFolder: (jobId: string) => call<void>("open_job_folder", { jobId }),
  updateSettings: (patch: SettingsPatch) => call<Settings>("update_settings", { patch }),
  pickOutputFolder: () => call<Settings>("pick_output_folder"),
  pickFfmpeg: () => call<Settings>("pick_ffmpeg"),
  recheckTools: () => call<void>("recheck_tools"),
  clearHistory: () => call<void>("clear_history"),
  readdHistory: (entryId: string) => call<number>("readd_history", { entryId }),
  openHistoryFolder: (entryId: string) => call<void>("open_history_folder", { entryId }),
  openLogsFolder: () => call<void>("open_logs_folder"),
  openExternal: (link: ExternalLink) => call<void>("open_external", { link }),
};
