import { AlertTriangle, ArrowDown, ArrowUp, CheckCircle2, CircleSlash, FolderPlus, FolderSearch, Loader2, Plus, Trash2, X, XCircle } from "lucide-react";
import { type KeyboardEvent, useMemo, useState } from "react";
import { plural, useT } from "../i18n";
import { api } from "../lib/api";
import type { FileInfo } from "../lib/bindings/FileInfo";
import type { PlanIssue } from "../lib/bindings/PlanIssue";
import { FORMATS, formatLabel } from "../lib/formats";
import { accepts } from "../lib/operations";
import { describeDetails, formatBytes, percent } from "../lib/units";
import { fileTask, type TaskState, useApp } from "../store/app";
import { ErrorMessage, errorTitleKey } from "./ErrorMessage";
import { familyColor, Thumbnail } from "./FileIcon";
import { Button, Checkbox, cn, IconButton, ProgressBar } from "./ui/primitives";

export function FileQueue() {
  const t = useT();
  const files = useApp((s) => s.files);
  const selection = useApp((s) => s.selection);
  const setSelection = useApp((s) => s.setSelection);
  const removeFiles = useApp((s) => s.removeFiles);
  const clearFiles = useApp((s) => s.clearFiles);
  const announce = useApp((s) => s.announce);
  const job = useApp((s) => s.job);
  const inspecting = useApp((s) => s.inspecting);
  const truncated = useApp((s) => s.truncated);
  const running = job?.status === "running" || job?.status === "cancelling";
  const totalSize = useMemo(() => files.reduce((sum, f) => sum + f.size, 0), [files]);
  const allSelected = selection.length > 0 && selection.length === files.length;

  const removeSelected = () => {
    void removeFiles(selection);
    announce(t("live.removed"));
  };

  const onKeyDown = (event: KeyboardEvent<HTMLUListElement>) => {
    if (event.key !== "Delete" && event.key !== "Backspace") return;
    if (running) return;
    const row = (event.target as HTMLElement).closest<HTMLElement>("[data-file-id]");
    const target = (event.target as HTMLElement).tagName;
    if (!row || target === "INPUT") return;
    event.preventDefault();
    const ids = selection.length > 0 ? selection : [row.dataset.fileId!];
    void removeFiles(ids);
    announce(t("live.removed"));
  };

  return (
    <section aria-labelledby="queue-title" className="flex min-h-0 flex-col overflow-hidden rounded-2xl border border-border bg-surface">
      <div className="flex shrink-0 flex-wrap items-center gap-3 border-b border-border px-4 py-3">
        <Checkbox
          label={t("queue.selectAll")}
          checked={allSelected ? true : selection.length > 0 ? "indeterminate" : false}
          onChange={(checked) => setSelection(checked ? files.map((f) => f.id) : [])}
        />
        <div className="min-w-0 flex-1">
          <h2 id="queue-title" className="font-display text-[17px] leading-tight font-bold">
            {t("queue.title")}
          </h2>
          <p className="tabular text-xs text-muted">
            {t(plural("queue.summary", files.length), { count: files.length, size: formatBytes(totalSize, t.locale) })}
            {inspecting.active && (
              <span role="status" className="ml-2 inline-flex items-center gap-1 text-accent-ink">
                <Loader2 className="size-3 animate-spin" aria-hidden />
                {inspecting.count === 1 ? t("drop.inspectingOne") : t("drop.inspecting", { count: inspecting.count })}
              </span>
            )}
          </p>
          {truncated && (
            <p className="mt-0.5 flex items-center gap-1 text-xs font-medium text-warning" role="status">
              <AlertTriangle className="size-3.5 shrink-0" aria-hidden />
              {t("drop.truncated")}
            </p>
          )}
        </div>
        {selection.length > 0 ? (
          <Button size="sm" variant="danger" icon={<Trash2 className="size-4" aria-hidden />} onClick={removeSelected} disabled={running}>
            {t("queue.removeSelected", { count: selection.length })}
          </Button>
        ) : (
          <div className="flex items-center gap-1">
            <IconButton label={t("queue.addFiles")} onClick={() => api.pickFiles()}>
              <Plus className="size-[18px]" />
            </IconButton>
            <IconButton label={t("queue.addFolder")} onClick={() => api.pickFolder()}>
              <FolderPlus className="size-[18px]" />
            </IconButton>
            <IconButton label={t("queue.clearAll")} onClick={() => void clearFiles()} disabled={running}>
              <Trash2 className="size-[18px]" />
            </IconButton>
          </div>
        )}
      </div>
      <ul className="min-h-0 flex-1 overflow-y-auto py-1" onKeyDown={onKeyDown} aria-label={t("queue.title")}>
        {files.map((file, index) => (
          <FileRow key={file.id} file={file} index={index} total={files.length} />
        ))}
      </ul>
    </section>
  );
}

function FileRow({ file, index, total }: { file: FileInfo; index: number; total: number }) {
  const t = useT();
  const selected = useApp((s) => s.selection.includes(file.id));
  const toggleSelected = useApp((s) => s.toggleSelected);
  const removeFiles = useApp((s) => s.removeFiles);
  const moveFile = useApp((s) => s.moveFile);
  const operation = useApp((s) => s.operation);
  const capability = useApp((s) => s.tools.capabilities.find((c) => c.id === s.operation));
  const issue = useApp((s) => s.preview.plan?.skipped.find((i) => i.fileId === file.id));
  const job = useApp((s) => s.job);
  const cancelTask = useApp((s) => s.cancelTask);
  const task = fileTask(job, file.id);
  const jobSkipped = job?.skipped.find((i) => i.fileId === file.id);
  const running = job?.status === "running" || job?.status === "cancelling";
  const applicable = !capability || accepts(capability, file);
  const details = describeDetails(file.details, t.locale, t);
  const reorderable = operation === "pdfMerge" && applicable && !running && total > 1;

  return (
    <li
      data-file-id={file.id}
      className={cn(
        "group/row flex items-start gap-3 border-b border-border/70 px-4 py-2.5 last:border-b-0 [contain-intrinsic-size:64px] [content-visibility:auto]",
        selected && "bg-accent-soft/60",
      )}
    >
      <Checkbox
        className="mt-3.5"
        label={t("queue.select", { name: file.name })}
        checked={selected}
        onChange={() => toggleSelected(file.id)}
      />
      <div className={cn(!applicable && !task && "opacity-50")}>
        <Thumbnail file={file} />
      </div>
      <div className={cn("min-w-0 flex-1 pt-0.5", !applicable && !task && "opacity-60")}>
        <p className="truncate text-sm font-semibold" title={file.displayPath}>
          {file.name}
        </p>
        <div className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted">
          <span
            className="rounded-md px-1.5 py-px text-[11px] font-bold tracking-wide"
            style={{ color: familyColor(file.family), background: `color-mix(in oklab, ${familyColor(file.family)} 12%, transparent)` }}
          >
            {file.format ? FORMATS[file.format].label : t("queue.unknownFormat")}
          </span>
          {details && <span className="tabular">{details}</span>}
          <span className="tabular">{formatBytes(file.size, t.locale)}</span>
          {file.details?.kind === "image" && file.details.animated && <span>{t("queue.animated")}</span>}
        </div>
        {file.extensionMismatch && file.format && (
          <p className="mt-1 flex items-center gap-1 text-xs font-medium text-warning">
            <AlertTriangle className="size-3.5 shrink-0" aria-hidden />
            {t("queue.mismatch", { format: formatLabel(file.format), ext: file.extension ?? "" })}
          </p>
        )}
        <RowStatus file={file} task={task} issue={issue ?? jobSkipped} applicable={applicable} />
      </div>
      <div className="flex shrink-0 items-center gap-0.5 pt-1.5">
        {reorderable && (
          <>
            <IconButton label={t("queue.moveUp", { name: file.name })} disabled={index === 0} onClick={() => void moveFile(file.id, -1)} className="size-8">
              <ArrowUp className="size-4" />
            </IconButton>
            <IconButton label={t("queue.moveDown", { name: file.name })} disabled={index === total - 1} onClick={() => void moveFile(file.id, 1)} className="size-8">
              <ArrowDown className="size-4" />
            </IconButton>
          </>
        )}
        {task && running && (task.status === "queued" || task.status === "running") ? (
          <IconButton label={t("queue.cancelItem", { name: file.name })} onClick={() => void cancelTask(task.id)} className="size-8">
            <X className="size-4" />
          </IconButton>
        ) : task?.status === "succeeded" && task.outcome?.status === "succeeded" && task.outcome.outputs[0] ? (
          <IconButton
            label={t("queue.reveal", { name: task.outcome.outputs[0].name })}
            onClick={() => task.outcome?.status === "succeeded" && api.revealOutput(task.outcome.outputs[0].id)}
            className="size-8"
          >
            <FolderSearch className="size-4" />
          </IconButton>
        ) : (
          !running && (
            <IconButton label={t("queue.remove", { name: file.name })} onClick={() => void removeFiles([file.id])} className="size-8 opacity-70 group-hover/row:opacity-100">
              <X className="size-4" />
            </IconButton>
          )
        )}
      </div>
    </li>
  );
}

function RowStatus({ file, task, issue, applicable }: { file: FileInfo; task: TaskState | null; issue?: PlanIssue; applicable: boolean }) {
  const t = useT();
  const [open, setOpen] = useState(false);

  if (task) {
    switch (task.status) {
      case "queued":
        return <p className="mt-1 text-xs text-subtle">{t("queue.status.queued")}</p>;
      case "running":
        return (
          <div className="mt-1.5 flex items-center gap-2">
            <Loader2 className="size-3.5 shrink-0 animate-spin text-accent" aria-hidden />
            <ProgressBar value={task.progress} label={file.name} className="h-1.5 max-w-56 flex-1" />
            <span className="tabular text-xs text-muted">{percent(task.progress)}%</span>
          </div>
        );
      case "succeeded":
        return (
          <p className="mt-1 flex items-center gap-1 text-xs font-medium text-success">
            <CheckCircle2 className="size-3.5 shrink-0" aria-hidden />
            <span className="truncate">
              {task.outcome?.status === "succeeded" && task.outcome.outputs[0] ? `→ ${task.outcome.outputs[0].name}` : t("queue.status.succeeded")}
            </span>
          </p>
        );
      case "cancelled":
        return (
          <p className="mt-1 flex items-center gap-1 text-xs text-muted">
            <CircleSlash className="size-3.5" aria-hidden />
            {t("queue.status.cancelled")}
          </p>
        );
      case "failed":
      case "skipped": {
        const error = task.outcome && (task.outcome.status === "failed" || task.outcome.status === "skipped") ? task.outcome.error : null;
        return (
          <div className="mt-1">
            <button
              type="button"
              aria-expanded={open}
              onClick={() => setOpen(!open)}
              className={cn("flex items-center gap-1 text-left text-xs font-medium", task.status === "failed" ? "text-danger" : "text-warning")}
            >
              <XCircle className="size-3.5 shrink-0" aria-hidden />
              {error ? t(errorTitleKey(error.code)) : t(`queue.status.${task.status}`)}
              <span className="text-subtle underline">{t("queue.details")}</span>
            </button>
            {open && error && <ErrorMessage error={error} className="mt-1.5" />}
          </div>
        );
      }
    }
  }
  if (file.issue) {
    return <ErrorMessage error={file.issue} compact className="mt-1 text-xs" />;
  }
  if (issue && applicable) {
    return <ErrorMessage error={issue.error} compact className="mt-1 text-xs" />;
  }
  if (!applicable) {
    return <p className="mt-1 text-xs text-subtle">{t("queue.notCompatible")}</p>;
  }
  return null;
}
