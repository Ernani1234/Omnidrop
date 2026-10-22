import { AlertTriangle, CheckCircle2, CircleSlash, FolderOpen, ListChecks, Repeat, RotateCcw, SquarePlus, X, XCircle } from "lucide-react";
import { useT } from "../i18n";
import { api } from "../lib/api";
import { OPERATION_BY_ID } from "../lib/operations";
import { formatBytes, percent } from "../lib/units";
import { finishedCount, type JobState, jobProgress, useApp } from "../store/app";
import { Button, cn, IconButton, ProgressBar } from "./ui/primitives";

export function RunBar() {
  const job = useApp((s) => s.job);
  if (!job) return null;
  return (
    <div className="shrink-0 border-t border-border bg-surface px-5 py-3.5 shadow-card">
      {job.status === "finished" ? <Finished job={job} /> : <Running job={job} />}
    </div>
  );
}

function Running({ job }: { job: JobState }) {
  const t = useT();
  const cancelJob = useApp((s) => s.cancelJob);
  const total = job.order.length;
  const done = finishedCount(job);
  const progress = jobProgress(job);
  const meta = OPERATION_BY_ID[job.operation];
  const text = job.status === "cancelling" ? t("running.cancelling") : t(meta.running, { current: Math.min(total, done + 1), total });
  return (
    <div className="flex items-center gap-4">
      <div className="min-w-0 flex-1">
        <div className="mb-2 flex items-baseline justify-between gap-3">
          <p className="truncate text-sm font-semibold" role="status">
            {text}
          </p>
          <span className="tabular shrink-0 text-xs text-muted">{t("running.percent", { percent: percent(progress) })}</span>
        </div>
        <ProgressBar value={progress} label={text} className="h-2.5" />
      </div>
      <Button variant="danger" onClick={() => void cancelJob()} disabled={job.status === "cancelling"} icon={<X className="size-4" aria-hidden />}>
        {t("running.cancel")}
      </Button>
    </div>
  );
}

const SAVINGS_OPS = new Set(["imageConvert", "audioConvert", "videoConvert", "archiveCreate"]);

function Finished({ job }: { job: JobState }) {
  const t = useT();
  const startJob = useApp((s) => s.startJob);
  const dismissJob = useApp((s) => s.dismissJob);
  const clearFiles = useApp((s) => s.clearFiles);
  const openDialog = useApp((s) => s.openDialog);
  const summary = job.summary!;
  const total = job.order.length;
  const failedFileIds = job.order
    .map((id) => job.tasks[id])
    .filter((task) => task.status === "failed")
    .flatMap((task) => task.fileIds);

  let tone: "success" | "warning" | "danger" | "muted" = "success";
  let message = t("done.success");
  if (summary.cancelled > 0) {
    tone = "muted";
    message = summary.succeeded > 0 ? t("done.cancelled", { succeeded: summary.succeeded }) : t("done.cancelledNone");
  } else if (summary.succeeded === 0 && summary.failed > 0) {
    tone = "danger";
    message = t("done.failed");
  } else if (summary.failed > 0) {
    tone = "warning";
    message = t("done.partial", { succeeded: summary.succeeded, total, failed: summary.failed });
  }
  const saved = summary.inputBytes - summary.outputBytes;
  const showSavings = SAVINGS_OPS.has(job.operation) && summary.succeeded > 0 && summary.failed === 0 && saved > 0;
  const Icon = tone === "success" ? CheckCircle2 : tone === "warning" ? AlertTriangle : tone === "danger" ? XCircle : CircleSlash;

  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-3">
      <div className="flex min-w-0 flex-1 items-start gap-3" role="status">
        <Icon
          className={cn(
            "mt-0.5 size-6 shrink-0",
            tone === "success" && "text-success",
            tone === "warning" && "text-warning",
            tone === "danger" && "text-danger",
            tone === "muted" && "text-muted",
          )}
          aria-hidden
        />
        <div className="min-w-0">
          <p className="text-sm font-semibold">{message}</p>
          <p className="text-xs text-muted">
            {showSavings && <span className="font-medium text-success">{t("done.saved", { size: formatBytes(saved, t.locale) })}</span>}
            {showSavings && summary.skipped > 0 && " · "}
            {summary.skipped > 0 && t("done.skipped", { count: summary.skipped })}
          </p>
        </div>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        {summary.outputs.length > 0 && (
          <Button variant="primary" icon={<FolderOpen className="size-4" aria-hidden />} onClick={() => void api.openJobFolder(job.id)}>
            {t("done.openFolder")}
          </Button>
        )}
        <Button icon={<ListChecks className="size-4" aria-hidden />} onClick={() => openDialog("results")}>
          {t("done.viewResults")}
        </Button>
        {failedFileIds.length > 0 ? (
          <Button icon={<RotateCcw className="size-4" aria-hidden />} onClick={() => void startJob(job.request, failedFileIds)}>
            {t("done.retryFailed")}
          </Button>
        ) : (
          <Button icon={<Repeat className="size-4" aria-hidden />} onClick={() => void startJob(job.request, job.fileIds)}>
            {t("done.repeat")}
          </Button>
        )}
        <Button variant="ghost" icon={<SquarePlus className="size-4" aria-hidden />} onClick={() => void clearFiles()}>
          {t("done.newJob")}
        </Button>
        <IconButton label={t("done.dismiss")} onClick={dismissJob}>
          <X className="size-4" />
        </IconButton>
      </div>
    </div>
  );
}
