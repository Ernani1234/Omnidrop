import { RadioGroup } from "radix-ui";
import { ArrowRight, Info, ShieldCheck } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { type MessageKey, plural, useT } from "../i18n";
import { api } from "../lib/api";
import type { TaskNote } from "../lib/bindings/TaskNote";
import {
  archiveDefaultName,
  buildRequest,
  defaultOperation,
  type OperationChoice,
  operationsFor,
  withAvailableTargets,
} from "../lib/operations";
import { useApp } from "../store/app";
import { ErrorMessage, errorTitleKey, noteText } from "./ErrorMessage";
import { OperationForm } from "./forms/OperationForms";
import { Button, cn, NativeSelect, SectionLabel } from "./ui/primitives";

export function useCurrentRequest() {
  const t = useT();
  const files = useApp((s) => s.files);
  const operation = useApp((s) => s.operation);
  const options = useApp((s) => s.options);
  const capabilities = useApp((s) => s.tools.capabilities);
  const archiveName = useMemo(() => archiveDefaultName(files, t("archive.nameDefault")), [files, t]);
  const request = useMemo(() => {
    if (!operation) return null;
    const effective = withAvailableTargets(options, capabilities);
    return buildRequest(operation, effective, { mergeName: t("pdf.outputNameDefault"), archiveName });
  }, [operation, options, capabilities, t, archiveName]);
  return { request, archiveName };
}

export function OperationPanel() {
  const t = useT();
  const files = useApp((s) => s.files);
  const capabilities = useApp((s) => s.tools.capabilities);
  const operation = useApp((s) => s.operation);
  const setOperation = useApp((s) => s.setOperation);
  const openDialog = useApp((s) => s.openDialog);
  const job = useApp((s) => s.job);
  const running = job?.status === "running" || job?.status === "cancelling";
  const choices = useMemo(() => operationsFor(files, capabilities), [files, capabilities]);
  const { request, archiveName } = useCurrentRequest();

  // Keep a valid operation selected as files come and go.
  useEffect(() => {
    if (choices.length === 0) {
      if (operation) setOperation(null);
      return;
    }
    if (!operation || !choices.some((c) => c.meta.id === operation)) {
      setOperation(defaultOperation(choices));
    }
  }, [choices, operation, setOperation]);

  usePlanPreview(request);

  const selected = choices.find((c) => c.meta.id === operation);

  return (
    <section aria-labelledby="op-title" className="flex min-h-0 flex-col overflow-hidden rounded-2xl border border-border bg-surface">
      <div inert={running} className={cn("min-h-0 flex-1 overflow-y-auto px-5 pt-4 pb-5 transition-opacity", running && "opacity-55")}>
        <h2 id="op-title" className="font-display text-[19px] leading-tight font-bold tracking-tight">
          {t("op.question")}
        </h2>
        {choices.length === 0 ? (
          <div className="mt-4 rounded-xl bg-surface-2 p-4 text-sm">
            <p className="font-semibold">{t("op.none")}</p>
            <p className="mt-1 text-muted">{t("op.noneHint")}</p>
          </div>
        ) : (
          <OperationList choices={choices} value={operation} onChange={setOperation} />
        )}
        {selected && selected.capability.available && (
          <div className="mt-6 border-t border-border pt-5">
            <OperationForm operation={selected.meta.id} capability={selected.capability} archiveName={archiveName} />
            <OutputSection />
          </div>
        )}
        {selected && !selected.capability.available && selected.capability.unavailableReason && (
          <div className="mt-5 rounded-xl border border-warning/40 bg-warning-soft p-4">
            <ErrorMessage error={{ code: selected.capability.unavailableReason, detail: "" }} />
            <Button size="sm" className="mt-3" onClick={() => openDialog("settings")}>
              {t("header.settings")}
            </Button>
          </div>
        )}
      </div>
      {selected && <ActionFooter choice={selected} />}
    </section>
  );
}

function OperationList({ choices, value, onChange }: { choices: OperationChoice[]; value: string | null; onChange: (id: OperationChoice["meta"]["id"]) => void }) {
  const t = useT();
  return (
    <RadioGroup.Root
      aria-label={t("op.question")}
      value={value ?? ""}
      onValueChange={(v) => onChange(v as OperationChoice["meta"]["id"])}
      className="mt-3 grid gap-2"
    >
      {choices.map(({ meta, capability, count }) => {
        const Icon = meta.icon;
        const unavailable = !capability.available;
        const selected = meta.id === value;
        return (
          <RadioGroup.Item
            key={meta.id}
            value={meta.id}
            aria-describedby={`op-desc-${meta.id}`}
            className={cn(
              "group flex items-center gap-3 rounded-xl border border-border px-2.5 py-2 text-left transition-colors",
              "hover:border-border-strong data-[state=checked]:border-accent data-[state=checked]:bg-accent-soft",
            )}
          >
            <span
              className="flex size-8 shrink-0 items-center justify-center rounded-lg"
              style={{ color: `var(--fam-${meta.family})`, background: `color-mix(in oklab, var(--fam-${meta.family}) 13%, var(--surface))` }}
              aria-hidden
            >
              <Icon className="size-[17px]" />
            </span>
            <span className="min-w-0 flex-1">
              <span className="block text-sm leading-snug font-bold">{t(meta.title)}</span>
              <span
                id={`op-desc-${meta.id}`}
                className={cn(
                  "text-xs leading-snug",
                  unavailable ? "block font-medium text-warning" : "text-muted",
                  !unavailable && (selected ? "block" : "sr-only"),
                )}
              >
                {unavailable && capability.unavailableReason ? t(errorTitleKey(capability.unavailableReason)) : t(meta.description)}
              </span>
            </span>
            <span className="tabular shrink-0 rounded-full bg-surface-2 px-2 py-0.5 text-[11px] font-semibold text-muted group-data-[state=checked]:bg-surface">
              {t(plural("op.filesCount", count), { count })}
            </span>
          </RadioGroup.Item>
        );
      })}
    </RadioGroup.Root>
  );
}

function OutputSection() {
  const t = useT();
  const settings = useApp((s) => s.settings);
  const setSettings = useApp((s) => s.setSettings);
  const updateSettings = useApp((s) => s.updateSettings);
  const folderName = settings.outputFolder?.split(/[\\/]/).filter(Boolean).pop();
  const value = settings.outputMode === "folder" && settings.outputFolder ? "folder" : "nextToSource";

  return (
    <div className="mt-6 grid gap-3 border-t border-border pt-5">
      <SectionLabel>{t("output.title")}</SectionLabel>
      <NativeSelect
        aria-label={t("output.title")}
        value={value}
        onChange={(e) => {
          const next = e.target.value;
          if (next === "choose") {
            e.target.value = value;
            void api.pickOutputFolder().then(setSettings);
          } else {
            void updateSettings({ outputMode: next as "nextToSource" | "folder" });
          }
        }}
      >
        <option value="nextToSource">{t("output.nextToSource")}</option>
        {settings.outputFolder && <option value="folder">{folderName}</option>}
        <option value="choose">{settings.outputFolder ? t("output.changeFolder") : t("output.chooseFolder")}</option>
      </NativeSelect>
      {value === "folder" && settings.outputFolder && (
        <p className="-mt-1 truncate text-xs text-muted" title={settings.outputFolder}>
          {settings.outputFolder}
        </p>
      )}
      <label className="grid gap-1.5">
        <span className="text-[13px] font-semibold">{t("output.collision")}</span>
        <NativeSelect value={settings.collision} onChange={(e) => void updateSettings({ collision: e.target.value as typeof settings.collision })}>
          <option value="rename">{t("output.collision.rename")}</option>
          <option value="overwrite">{t("output.collision.overwrite")}</option>
          <option value="skip">{t("output.collision.skip")}</option>
        </NativeSelect>
      </label>
      <p className="flex items-center gap-1.5 text-xs text-muted">
        <ShieldCheck className="size-3.5 shrink-0 text-success" aria-hidden />
        {t("output.originalsSafe")}
      </p>
    </div>
  );
}

function usePlanPreview(request: ReturnType<typeof useCurrentRequest>["request"]) {
  const filesKey = useApp((s) => s.files.map((f) => f.id).join("|"));
  const settings = useApp((s) => s.settings);
  const setPreview = useApp((s) => s.setPreview);
  const requestKey = JSON.stringify(request);

  useEffect(() => {
    if (!request || !filesKey) {
      setPreview({ plan: null, error: null, loading: false });
      return;
    }
    let cancelled = false;
    setPreview({ loading: true });
    const timer = window.setTimeout(() => {
      api
        .previewPlan(request)
        .then((plan) => !cancelled && setPreview({ plan, error: null, loading: false }))
        .catch((error) => !cancelled && setPreview({ plan: null, error, loading: false }));
    }, 120);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
    // The serialized request captures every option that matters.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [requestKey, filesKey, settings.outputMode, settings.outputFolder, settings.collision, setPreview]);
}

function summarizeNotes(notes: TaskNote[]): TaskNote[] {
  const seen = new Map<string, TaskNote>();
  for (const note of notes) if (!seen.has(note.code)) seen.set(note.code, note);
  return [...seen.values()];
}

function ActionFooter({ choice }: { choice: OperationChoice }) {
  const t = useT();
  const preview = useApp((s) => s.preview);
  const job = useApp((s) => s.job);
  const startJob = useApp((s) => s.startJob);
  const startError = useApp((s) => s.startError);
  const platform = useApp((s) => s.platform);
  const { request } = useCurrentRequest();
  const [showSkipped, setShowSkipped] = useState(false);
  const running = job?.status === "running" || job?.status === "cancelling";
  const plan = preview.plan;
  const inputCount = plan ? plan.tasks.reduce((sum, task) => sum + task.fileIds.length, 0) : 0;
  const canStart = !!plan && plan.tasks.length > 0 && !running && !preview.loading && choice.capability.available;
  const notes = plan ? summarizeNotes(plan.tasks.flatMap((task) => task.notes)) : [];
  const label = canStart
    ? t(plural("cta.files", inputCount), { verb: t(choice.meta.verb), count: inputCount })
    : t(plural("cta.files", choice.count), { verb: t(choice.meta.verb), count: choice.count });

  const start = () => {
    if (canStart && request) void startJob(request);
  };

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
        event.preventDefault();
        start();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  return (
    <div className="shrink-0 border-t border-border bg-surface px-5 py-4">
      <div className="mb-3 grid gap-1 text-[13px]" aria-live="polite">
        {preview.error ? (
          <ErrorMessage error={preview.error} />
        ) : plan ? (
          <>
            {plan.expectedOutputs != null && plan.tasks.length > 0 && (
              <p className="text-muted">{t(plural("plan.outputs", plan.expectedOutputs), { count: plan.expectedOutputs })}</p>
            )}
            {notes.map((note) => (
              <p key={note.code} className="flex items-start gap-1.5 text-muted">
                <Info className="mt-0.5 size-3.5 shrink-0 text-accent" aria-hidden />
                {noteText(t, note)}
              </p>
            ))}
            {plan.skipped.length > 0 && (
              <div>
                <button type="button" className="font-medium text-warning hover:underline" aria-expanded={showSkipped} onClick={() => setShowSkipped(!showSkipped)}>
                  {t(plural("plan.skipped", plan.skipped.length), { count: plan.skipped.length })} · {t("plan.why")}
                </button>
                {showSkipped && (
                  <ul className="mt-1.5 grid max-h-32 gap-1 overflow-y-auto">
                    {plan.skipped.map((issue) => (
                      <li key={issue.fileId} className="text-xs text-muted">
                        <span className="font-semibold text-text">{issue.name}</span>: {t(errorTitleKey(issue.error.code) as MessageKey)}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            )}
          </>
        ) : (
          preview.loading && <p className="text-muted">{t("plan.checking")}</p>
        )}
        {startError && <ErrorMessage error={startError} />}
      </div>
      <Button
        variant="primary"
        size="lg"
        className="w-full"
        disabled={!canStart}
        onClick={start}
        aria-keyshortcuts="Control+Enter"
        icon={<ArrowRight className="order-last size-5" aria-hidden />}
      >
        {label}
      </Button>
      <p className="mt-1.5 text-center text-[11px] text-subtle">{platform === "macos" ? "⌘ Enter" : t("cta.shortcut")}</p>
    </div>
  );
}
