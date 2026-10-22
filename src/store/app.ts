import { create } from "zustand";
import { api } from "../lib/api";
import type { Bootstrap } from "../lib/bindings/Bootstrap";
import type { EngineError } from "../lib/bindings/EngineError";
import type { FileInfo } from "../lib/bindings/FileInfo";
import type { HistoryEntry } from "../lib/bindings/HistoryEntry";
import type { InspectingEvent } from "../lib/bindings/InspectingEvent";
import type { JobEvent } from "../lib/bindings/JobEvent";
import type { JobSummary } from "../lib/bindings/JobSummary";
import type { OperationId } from "../lib/bindings/OperationId";
import type { OperationRequest } from "../lib/bindings/OperationRequest";
import type { Plan } from "../lib/bindings/Plan";
import type { PlanIssue } from "../lib/bindings/PlanIssue";
import type { Settings } from "../lib/bindings/Settings";
import type { SettingsPatch } from "../lib/bindings/SettingsPatch";
import type { TaskOutcome } from "../lib/bindings/TaskOutcome";
import type { ToolsPayload } from "../lib/bindings/ToolsPayload";
import { DEFAULT_OPTIONS, type OperationOptions } from "../lib/operations";

export type TaskStatus = "queued" | "running" | "succeeded" | "failed" | "skipped" | "cancelled";

export interface TaskState {
  id: string;
  fileIds: string[];
  label: string;
  outputName: string;
  status: TaskStatus;
  progress: number;
  outcome: TaskOutcome | null;
}

export interface JobState {
  id: string;
  operation: OperationId;
  request: OperationRequest;
  fileIds: string[];
  tasks: Record<string, TaskState>;
  order: string[];
  skipped: PlanIssue[];
  status: "running" | "cancelling" | "finished";
  summary: JobSummary | null;
}

export type DialogName = "settings" | "results" | "history" | null;

export interface AppStore {
  ready: boolean;
  version: string;
  platform: string;
  settings: Settings;
  tools: ToolsPayload;
  history: HistoryEntry[];
  files: FileInfo[];
  selection: string[];
  inspecting: InspectingEvent;
  dragActive: boolean;
  truncated: boolean;
  thumbnails: Record<string, string | null>;
  operation: OperationId | null;
  options: OperationOptions;
  preview: { plan: Plan | null; error: EngineError | null; loading: boolean };
  job: JobState | null;
  dialog: DialogName;
  announcement: { text: string; id: number };
  startError: EngineError | null;

  hydrate: (bootstrap: Bootstrap) => void;
  addFiles: (files: FileInfo[], truncated: boolean) => void;
  removeFiles: (ids: string[]) => Promise<void>;
  clearFiles: () => Promise<void>;
  moveFile: (id: string, delta: number) => Promise<void>;
  setSelection: (ids: string[]) => void;
  toggleSelected: (id: string) => void;
  setInspecting: (event: InspectingEvent) => void;
  setDragActive: (active: boolean) => void;
  setThumbnail: (id: string, url: string | null) => void;
  setOperation: (operation: OperationId | null) => void;
  setOptions: <K extends keyof OperationOptions>(operation: K, patch: Partial<OperationOptions[K]>) => void;
  setPreview: (preview: Partial<AppStore["preview"]>) => void;
  startJob: (request: OperationRequest, fileIds?: string[]) => Promise<void>;
  handleJobEvent: (event: JobEvent) => void;
  cancelJob: () => Promise<void>;
  cancelTask: (taskId: string) => Promise<void>;
  dismissJob: () => void;
  setTools: (tools: ToolsPayload) => void;
  setSettings: (settings: Settings) => void;
  updateSettings: (patch: SettingsPatch) => Promise<void>;
  refreshHistory: () => Promise<void>;
  openDialog: (dialog: DialogName) => void;
  announce: (text: string) => void;
}

const EMPTY_TOOLS: ToolsPayload = { ready: false, tools: { ffmpeg: null, ffprobe: null }, capabilities: [] };

export const DEFAULT_SETTINGS: Settings = {
  theme: "system",
  language: "auto",
  outputMode: "nextToSource",
  outputFolder: null,
  collision: "rename",
  ffmpegPath: null,
};

const statusOf = (outcome: TaskOutcome): TaskStatus => outcome.status;

export const useApp = create<AppStore>()((set, get) => ({
  ready: false,
  version: "",
  platform: "",
  settings: DEFAULT_SETTINGS,
  tools: EMPTY_TOOLS,
  history: [],
  files: [],
  selection: [],
  inspecting: { active: false, count: 0 },
  dragActive: false,
  truncated: false,
  thumbnails: {},
  operation: null,
  options: DEFAULT_OPTIONS,
  preview: { plan: null, error: null, loading: false },
  job: null,
  dialog: null,
  announcement: { text: "", id: 0 },
  startError: null,

  hydrate: (b) =>
    set({
      ready: true,
      version: b.version,
      platform: b.platform,
      settings: b.settings,
      tools: b.tools,
      history: b.history,
      files: b.files,
    }),

  addFiles: (files, truncated) =>
    set((s) => ({
      files: [...s.files, ...files.filter((f) => !s.files.some((existing) => existing.id === f.id))],
      truncated: truncated || s.truncated,
    })),

  removeFiles: async (ids) => {
    set((s) => ({
      files: s.files.filter((f) => !ids.includes(f.id)),
      selection: s.selection.filter((id) => !ids.includes(id)),
    }));
    await api.removeFiles(ids);
    if (get().files.length === 0) set({ operation: null, truncated: false });
  },

  clearFiles: async () => {
    set({ files: [], selection: [], operation: null, truncated: false, job: null, thumbnails: {} });
    await api.clearFiles();
  },

  moveFile: async (id, delta) => {
    const files = [...get().files];
    const from = files.findIndex((f) => f.id === id);
    const to = from + delta;
    if (from < 0 || to < 0 || to >= files.length) return;
    const [file] = files.splice(from, 1);
    files.splice(to, 0, file);
    set({ files });
    await api.moveFile(id, to);
  },

  setSelection: (ids) => set({ selection: ids }),
  toggleSelected: (id) =>
    set((s) => ({
      selection: s.selection.includes(id) ? s.selection.filter((x) => x !== id) : [...s.selection, id],
    })),
  setInspecting: (inspecting) => set({ inspecting }),
  setDragActive: (dragActive) => set({ dragActive }),
  setThumbnail: (id, url) => set((s) => ({ thumbnails: { ...s.thumbnails, [id]: url } })),
  setOperation: (operation) => set({ operation, startError: null }),
  setOptions: (operation, patch) =>
    set((s) => ({ options: { ...s.options, [operation]: { ...s.options[operation], ...patch } } })),
  setPreview: (preview) => set((s) => ({ preview: { ...s.preview, ...preview } })),

  startJob: async (request, fileIds) => {
    set({ startError: null });
    try {
      const plan = await api.startJob(request, fileIds);
      const tasks: Record<string, TaskState> = {};
      for (const task of plan.tasks) {
        tasks[task.id] = {
          id: task.id,
          fileIds: task.fileIds,
          label: task.label,
          outputName: task.outputName,
          status: "queued",
          progress: 0,
          outcome: null,
        };
      }
      set({
        job: {
          id: plan.jobId,
          operation: plan.operation,
          request,
          fileIds: plan.tasks.flatMap((t) => t.fileIds),
          tasks,
          order: plan.tasks.map((t) => t.id),
          skipped: plan.skipped,
          status: "running",
          summary: null,
        },
      });
    } catch (error) {
      set({ startError: error as EngineError });
    }
  },

  handleJobEvent: (event) =>
    set((s) => {
      const job = s.job;
      if (!job || job.id !== event.jobId) return {};
      switch (event.type) {
        case "jobStarted":
          return {};
        case "taskStarted":
        case "taskProgress": {
          const task = job.tasks[event.taskId];
          if (!task || task.status !== "queued" && task.status !== "running") return {};
          const progress = event.type === "taskProgress" ? event.progress : task.progress;
          return {
            job: { ...job, tasks: { ...job.tasks, [task.id]: { ...task, status: "running", progress } } },
          };
        }
        case "taskFinished": {
          const task = job.tasks[event.taskId];
          if (!task) return {};
          const status = statusOf(event.outcome);
          return {
            job: {
              ...job,
              tasks: { ...job.tasks, [task.id]: { ...task, status, progress: 1, outcome: event.outcome } },
            },
          };
        }
        case "jobFinished":
          return { job: { ...job, status: "finished", summary: event.summary } };
      }
    }),

  cancelJob: async () => {
    const job = get().job;
    if (!job || job.status !== "running") return;
    set({ job: { ...job, status: "cancelling" } });
    await api.cancelJob(job.id).catch(() => undefined);
  },

  cancelTask: async (taskId) => {
    const job = get().job;
    if (!job) return;
    await api.cancelTask(job.id, taskId).catch(() => undefined);
  },

  dismissJob: () => set({ job: null }),
  setTools: (tools) => set({ tools }),
  setSettings: (settings) => set({ settings }),
  updateSettings: async (patch) => {
    const settings = await api.updateSettings(patch);
    set({ settings });
  },
  refreshHistory: async () => {
    const bootstrap = await api.bootstrap();
    set({ history: bootstrap.history });
  },
  openDialog: (dialog) => set({ dialog }),
  announce: (text) => set((s) => ({ announcement: { text, id: s.announcement.id + 1 } })),
}));

/** Aggregate progress of a job in 0..1. */
export function jobProgress(job: JobState): number {
  if (job.order.length === 0) return 1;
  const total = job.order.reduce((sum, id) => {
    const task = job.tasks[id];
    return sum + (task.status === "queued" ? 0 : task.status === "running" ? task.progress : 1);
  }, 0);
  return total / job.order.length;
}

export function finishedCount(job: JobState): number {
  return job.order.filter((id) => {
    const status = job.tasks[id].status;
    return status !== "queued" && status !== "running";
  }).length;
}

/** Status of a file in the current job, if it is part of it. */
export function fileTask(job: JobState | null, fileId: string): TaskState | null {
  if (!job) return null;
  for (const id of job.order) {
    const task = job.tasks[id];
    if (task.fileIds.includes(fileId)) return task;
  }
  return null;
}
