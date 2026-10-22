import { CodeXml, ExternalLink, FileText, FolderOpen, FolderSearch, RefreshCw, RotateCcw, Trash2 } from "lucide-react";
import { type MessageKey, useT } from "../i18n";
import { api } from "../lib/api";
import type { Language } from "../lib/bindings/Language";
import type { Theme } from "../lib/bindings/Theme";
import { FORMATS } from "../lib/formats";
import { OPERATION_BY_ID } from "../lib/operations";
import { formatBytes } from "../lib/units";
import { useApp } from "../store/app";
import { ErrorMessage, noteText } from "./ErrorMessage";
import { Button, Dialog, Field, IconButton, NativeSelect, SectionLabel, Segmented } from "./ui/primitives";

export function Dialogs() {
  const dialog = useApp((s) => s.dialog);
  const openDialog = useApp((s) => s.openDialog);
  const close = (open: boolean) => !open && openDialog(null);
  return (
    <>
      <ResultsDialog open={dialog === "results"} onOpenChange={close} />
      <SettingsDialog open={dialog === "settings"} onOpenChange={close} />
      <HistoryDialog open={dialog === "history"} onOpenChange={close} />
    </>
  );
}

interface DialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

function ResultsDialog({ open, onOpenChange }: DialogProps) {
  const t = useT();
  const job = useApp((s) => s.job);
  const files = useApp((s) => s.files);
  if (!job?.summary) return null;
  const tasks = job.order.map((id) => job.tasks[id]);
  const failures = tasks.filter((task) => task.status === "failed" || task.status === "skipped");
  const notes = tasks.flatMap((task) => (task.outcome?.status === "succeeded" ? task.outcome.notes : []));
  const uniqueNotes = notes.filter((note, i) => notes.findIndex((n) => n.code === note.code) === i);
  const sizeOf = (fileIds: string[]) => files.filter((f) => fileIds.includes(f.id)).reduce((sum, f) => sum + f.size, 0);

  return (
    <Dialog open={open} onOpenChange={onOpenChange} title={t("results.title")} description={t("results.description")} closeLabel={t("results.close")} wide>
      {job.summary.outputs.length === 0 ? (
        <p className="text-sm text-muted">{t("results.empty")}</p>
      ) : (
        <ul className="grid gap-1.5">
          {tasks.map((task) => {
            if (task.outcome?.status !== "succeeded") return null;
            return task.outcome.outputs.map((output) => {
              const before = task.fileIds.length === 1 ? sizeOf(task.fileIds) : 0;
              const change = before > 0 && !output.isDir ? Math.round(((output.size - before) / before) * 100) : null;
              return (
                <li key={output.id} className="flex items-center gap-3 rounded-xl border border-border px-3 py-2">
                  {output.isDir ? <FolderOpen className="size-5 shrink-0 text-muted" aria-hidden /> : <FileText className="size-5 shrink-0 text-muted" aria-hidden />}
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-semibold" title={output.displayPath}>
                      {output.name}
                    </p>
                    <p className="tabular text-xs text-muted">
                      {formatBytes(output.size, t.locale)}
                      {change != null && (
                        <span className={change <= 0 ? "text-success" : undefined}>
                          {" "}
                          · {change > 0 ? "+" : ""}
                          {change}%
                        </span>
                      )}
                      {output.renamed && (
                        <span className="ml-2 rounded bg-warning-soft px-1.5 py-px font-semibold text-warning" title={t("results.renamedHint")}>
                          {t("results.renamed")}
                        </span>
                      )}
                    </p>
                  </div>
                  {!output.isDir && (
                    <Button size="sm" onClick={() => void api.openOutput(output.id)}>
                      {t("results.open")}
                    </Button>
                  )}
                  <IconButton label={t("results.reveal")} onClick={() => void api.revealOutput(output.id)}>
                    <FolderSearch className="size-4" />
                  </IconButton>
                </li>
              );
            });
          })}
        </ul>
      )}
      {uniqueNotes.length > 0 && (
        <ul className="mt-4 grid gap-1 text-xs text-muted">
          {uniqueNotes.map((note) => (
            <li key={note.code}>• {noteText(t, note)}</li>
          ))}
        </ul>
      )}
      {failures.length > 0 && (
        <div className="mt-6">
          <SectionLabel>{t("results.failures")}</SectionLabel>
          <ul className="grid gap-3">
            {failures.map((task) => (
              <li key={task.id} className="rounded-xl border border-border px-3 py-2.5">
                <p className="mb-1 text-sm font-semibold">{task.label}</p>
                {task.outcome && (task.outcome.status === "failed" || task.outcome.status === "skipped") && <ErrorMessage error={task.outcome.error} />}
              </li>
            ))}
          </ul>
        </div>
      )}
    </Dialog>
  );
}

function SettingsDialog({ open, onOpenChange }: DialogProps) {
  const t = useT();
  const settings = useApp((s) => s.settings);
  const setSettings = useApp((s) => s.setSettings);
  const updateSettings = useApp((s) => s.updateSettings);
  const tools = useApp((s) => s.tools);
  const version = useApp((s) => s.version);
  const platform = useApp((s) => s.platform);
  const ffmpeg = tools.tools.ffmpeg;
  const missingFormats = [
    ...new Set(tools.capabilities.flatMap((c) => c.unavailableOutputFormats).map((f) => FORMATS[f].label)),
  ];
  const installHint =
    platform === "windows" ? "winget install Gyan.FFmpeg" : platform === "macos" ? "brew install ffmpeg" : "sudo apt install ffmpeg";

  return (
    <Dialog open={open} onOpenChange={onOpenChange} title={t("settings.title")} description={t("settings.description")} closeLabel={t("settings.close")}>
      <div className="grid gap-7">
        <section className="grid gap-4">
          <SectionLabel>{t("settings.appearance")}</SectionLabel>
          <Field label={t("settings.theme")}>
            <Segmented<Theme>
              label={t("settings.theme")}
              value={settings.theme}
              onChange={(theme) => void updateSettings({ theme })}
              choices={[
                { value: "system", label: t("settings.theme.system") },
                { value: "light", label: t("settings.theme.light") },
                { value: "dark", label: t("settings.theme.dark") },
              ]}
            />
          </Field>
          <Field label={t("settings.language")}>
            <NativeSelect aria-label={t("settings.language")} value={settings.language} onChange={(e) => void updateSettings({ language: e.target.value as Language })}>
              <option value="auto">{t("settings.language.auto")}</option>
              <option value="pt-BR">Português (Brasil)</option>
              <option value="en">English</option>
            </NativeSelect>
          </Field>
        </section>

        <section className="grid gap-4">
          <SectionLabel>{t("settings.output")}</SectionLabel>
          <Field label={t("output.title")}>
            <div className="flex gap-2">
              <NativeSelect
                aria-label={t("output.title")}
                value={settings.outputMode === "folder" && settings.outputFolder ? "folder" : "nextToSource"}
                onChange={(e) => void updateSettings({ outputMode: e.target.value as "folder" | "nextToSource" })}
                className="min-w-0"
              >
                <option value="nextToSource">{t("output.nextToSource")}</option>
                {settings.outputFolder && <option value="folder">{settings.outputFolder}</option>}
              </NativeSelect>
              <Button onClick={() => void api.pickOutputFolder().then(setSettings)}>{t("output.chooseFolder")}</Button>
            </div>
          </Field>
          <Field label={t("output.collision")} hint={t("output.originalsSafe")}>
            <NativeSelect aria-label={t("output.collision")} value={settings.collision} onChange={(e) => void updateSettings({ collision: e.target.value as typeof settings.collision })}>
              <option value="rename">{t("output.collision.rename")}</option>
              <option value="overwrite">{t("output.collision.overwrite")}</option>
              <option value="skip">{t("output.collision.skip")}</option>
            </NativeSelect>
          </Field>
        </section>

        <section className="grid gap-3">
          <SectionLabel>{t("settings.media")}</SectionLabel>
          <p className="text-sm font-semibold" role="status">
            {!tools.ready ? t("settings.ffmpegChecking") : ffmpeg ? t("settings.ffmpegFound", { version: ffmpeg.version }) : t("settings.ffmpegMissing")}
          </p>
          {ffmpeg && <p className="truncate text-xs text-muted" title={ffmpeg.path}>{ffmpeg.path}</p>}
          {ffmpeg && missingFormats.length > 0 && <p className="text-xs text-warning">{t("settings.encodersMissing", { formats: missingFormats.join(", ") })}</p>}
          <p className="text-[13px] text-muted">{t("settings.ffmpegWhy")}</p>
          {tools.ready && !ffmpeg && (
            <div className="rounded-xl bg-surface-2 p-3 text-[13px]">
              <p className="font-semibold">{t("settings.ffmpegInstall")}</p>
              <code className="mt-1 block rounded-lg bg-surface px-2 py-1.5 text-xs">{installHint}</code>
            </div>
          )}
          <div className="flex flex-wrap gap-2">
            <Button size="sm" icon={<RefreshCw className="size-4" aria-hidden />} onClick={() => void api.recheckTools()}>
              {t("settings.ffmpegRecheck")}
            </Button>
            <Button size="sm" onClick={() => void api.pickFfmpeg().then(setSettings)}>
              {t("settings.ffmpegLocate")}
            </Button>
            {settings.ffmpegPath && (
              <Button
                size="sm"
                variant="ghost"
                onClick={() => void updateSettings({ clearFfmpegPath: true }).then(() => api.recheckTools())}
              >
                {t("settings.ffmpegReset")}
              </Button>
            )}
            {!ffmpeg && (
              <Button size="sm" variant="ghost" icon={<ExternalLink className="size-4" aria-hidden />} onClick={() => void api.openExternal("ffmpegDownload")}>
                ffmpeg.org
              </Button>
            )}
          </div>
        </section>

        <section className="grid gap-2">
          <SectionLabel>{t("settings.privacy")}</SectionLabel>
          <p className="text-[13px] text-muted">{t("settings.privacyText")}</p>
        </section>

        <section className="grid gap-3">
          <SectionLabel>{t("settings.about")}</SectionLabel>
          <p className="text-sm">
            OmniDrop · {t("settings.version", { version })}
          </p>
          <div className="flex flex-wrap gap-2">
            <Button size="sm" icon={<FileText className="size-4" aria-hidden />} onClick={() => void api.openLogsFolder()}>
              {t("settings.logs")}
            </Button>
            <Button size="sm" icon={<CodeXml className="size-4" aria-hidden />} onClick={() => void api.openExternal("repository")}>
              {t("settings.repository")}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => void api.openExternal("issues")}>
              {t("settings.reportIssue")}
            </Button>
          </div>
        </section>
      </div>
    </Dialog>
  );
}

function HistoryDialog({ open, onOpenChange }: DialogProps) {
  const t = useT();
  const history = useApp((s) => s.history);
  const refreshHistory = useApp((s) => s.refreshHistory);
  const openDialog = useApp((s) => s.openDialog);
  const dateFormat = new Intl.DateTimeFormat(t.locale, { dateStyle: "medium", timeStyle: "short" });
  return (
    <Dialog open={open} onOpenChange={onOpenChange} title={t("history.title")} description={t("history.description")} closeLabel={t("common.close")} wide>
      {history.length === 0 ? (
        <p className="text-sm text-muted">{t("history.empty")}</p>
      ) : (
        <>
          <ul className="grid gap-2">
            {history.map((entry) => {
              const meta = OPERATION_BY_ID[entry.operation];
              const Icon = meta.icon;
              return (
                <li key={entry.id} className="flex flex-wrap items-center gap-3 rounded-xl border border-border px-3 py-2.5">
                  <Icon className="size-5 shrink-0" style={{ color: `var(--fam-${meta.family})` }} aria-hidden />
                  <div className="min-w-0 flex-1">
                    <p className="text-sm font-semibold">{t(meta.title as MessageKey)}</p>
                    <p className="text-xs text-muted">
                      {dateFormat.format(new Date(entry.finishedAt))} · {t("history.summary", { succeeded: entry.succeeded })}
                      {entry.failed > 0 && ` · ${t("history.failed", { count: entry.failed })}`}
                    </p>
                  </div>
                  {entry.outputDir && (
                    <Button size="sm" icon={<FolderOpen className="size-4" aria-hidden />} onClick={() => void api.openHistoryFolder(entry.id).catch(() => undefined)}>
                      {t("history.openFolder")}
                    </Button>
                  )}
                  <Button
                    size="sm"
                    variant="ghost"
                    icon={<RotateCcw className="size-4" aria-hidden />}
                    onClick={() => {
                      void api.readdHistory(entry.id);
                      openDialog(null);
                    }}
                  >
                    {t("history.readd")}
                  </Button>
                </li>
              );
            })}
          </ul>
          <Button className="mt-4" size="sm" variant="danger" icon={<Trash2 className="size-4" aria-hidden />} onClick={() => void api.clearHistory().then(refreshHistory)}>
            {t("history.clear")}
          </Button>
        </>
      )}
    </Dialog>
  );
}
