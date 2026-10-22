import { Archive, ArrowDownToLine, Clock, FileAudio, FileImage, FileText, FolderOpen, Lock, Plus } from "lucide-react";
import type { ReactNode } from "react";
import { useT } from "../i18n";
import { api } from "../lib/api";
import type { OperationId } from "../lib/bindings/OperationId";
import { useApp } from "../store/app";
import { Button, cn } from "./ui/primitives";

export function shortcut(platform: string, key: string) {
  return platform === "macos" ? `⌘${key}` : `Ctrl+${key}`;
}

export function EmptyState() {
  const t = useT();
  const platform = useApp((s) => s.platform);
  const dragActive = useApp((s) => s.dragActive);
  const inspecting = useApp((s) => s.inspecting);

  return (
    <div className="flex h-full min-h-0 flex-col items-center overflow-y-auto px-6 py-8">
      <section
        aria-labelledby="drop-title"
        className={cn(
          "relative flex w-full max-w-3xl flex-1 flex-col items-center justify-center rounded-[28px] border-2 border-dashed px-8 py-12 text-center transition-colors",
          dragActive ? "border-accent bg-accent-soft motion-safe:animate-[drop-pulse_1.6s_ease-in-out_infinite]" : "border-border-strong bg-surface",
        )}
        style={{ minHeight: 340 }}
      >
        <div className="mb-6 flex size-20 items-center justify-center rounded-[26px] bg-accent-soft text-accent">
          <ArrowDownToLine className="size-9" strokeWidth={2.2} aria-hidden />
        </div>
        <h1 id="drop-title" className="font-display text-[28px] leading-tight font-bold tracking-tight text-balance">
          {t("drop.title")}
        </h1>
        <p className="mt-2 max-w-md text-[15px] text-muted text-balance">{t("drop.subtitle")}</p>
        <div className="mt-7 flex flex-wrap items-center justify-center gap-3">
          <Button variant="primary" size="lg" icon={<Plus className="size-5" aria-hidden />} onClick={() => api.pickFiles()} aria-keyshortcuts="Control+O">
            {t("drop.pickFiles")}
          </Button>
          <Button size="lg" icon={<FolderOpen className="size-5" aria-hidden />} onClick={() => api.pickFolder()}>
            {t("drop.pickFolder")}
          </Button>
        </div>
        <p className="mt-3 text-xs text-subtle">{t("drop.shortcut", { keys: shortcut(platform, "O") })}</p>
        <p className="mt-6 flex items-center gap-1.5 text-[13px] font-medium text-success">
          <Lock className="size-3.5" aria-hidden />
          {t("drop.privacy")}
        </p>
        {inspecting.active && (
          <p className="mt-4 text-sm text-muted" role="status">
            {inspecting.count === 1 ? t("drop.inspectingOne") : t("drop.inspecting", { count: inspecting.count })}
          </p>
        )}
      </section>
      <QuickActions />
    </div>
  );
}

function QuickActions() {
  const t = useT();
  const capabilities = useApp((s) => s.tools.capabilities);
  const toolsReady = useApp((s) => s.tools.ready);
  const history = useApp((s) => s.history);
  const setOperation = useApp((s) => s.setOperation);
  const openDialog = useApp((s) => s.openDialog);
  const mediaAvailable = capabilities.some((c) => (c.id === "audioConvert" || c.id === "videoConvert") && c.available);

  const pick = (operation: OperationId | null, family: Parameters<typeof api.pickFiles>[0]) => {
    setOperation(operation);
    void api.pickFiles(family);
  };

  return (
    <nav aria-label={t("quick.title")} className="mt-6 grid w-full max-w-3xl grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-5">
      <QuickAction icon={<FileImage />} color="image" title={t("quick.images")} hint={t("quick.imagesHint")} onClick={() => pick("imageConvert", "image")} />
      <QuickAction icon={<FileText />} color="pdf" title={t("quick.pdf")} hint={t("quick.pdfHint")} onClick={() => pick(null, "pdf")} />
      <QuickAction
        icon={<FileAudio />}
        color="audio"
        title={t("quick.media")}
        hint={toolsReady && !mediaAvailable ? t("quick.mediaMissing") : t("quick.mediaHint")}
        warning={toolsReady && !mediaAvailable}
        onClick={() => (toolsReady && !mediaAvailable ? openDialog("settings") : pick(null, "audio"))}
      />
      <QuickAction icon={<Archive />} color="archive" title={t("quick.archives")} hint={t("quick.archivesHint")} onClick={() => pick(null, undefined)} />
      <QuickAction
        icon={<Clock />}
        color="unknown"
        title={t("quick.recent")}
        hint={history.length ? t("quick.recentHint") : t("quick.recentEmpty")}
        onClick={() => openDialog("history")}
      />
    </nav>
  );
}

function QuickAction({
  icon,
  color,
  title,
  hint,
  warning,
  onClick,
}: {
  icon: ReactNode;
  color: string;
  title: string;
  hint: string;
  warning?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="group flex flex-col items-start gap-2.5 rounded-2xl border border-border bg-surface p-4 text-left transition-colors hover:border-border-strong hover:bg-surface-2"
    >
      <span
        className="flex size-9 items-center justify-center rounded-xl [&>svg]:size-[18px]"
        style={{ color: `var(--fam-${color})`, background: `color-mix(in oklab, var(--fam-${color}) 13%, var(--surface))` }}
        aria-hidden
      >
        {icon}
      </span>
      <span>
        <span className="block text-sm leading-snug font-bold">{title}</span>
        <span className={cn("mt-0.5 block text-xs leading-snug", warning ? "font-semibold text-warning" : "text-muted")}>{hint}</span>
      </span>
    </button>
  );
}

export function DragOverlay() {
  const t = useT();
  const dragActive = useApp((s) => s.dragActive);
  const hasFiles = useApp((s) => s.files.length > 0);
  if (!dragActive || !hasFiles) return null;
  return (
    <div className="pointer-events-none fixed inset-0 z-30 flex items-center justify-center bg-overlay p-6" aria-hidden>
      <div className="flex w-full max-w-xl flex-col items-center rounded-[28px] border-2 border-dashed border-accent bg-surface px-8 py-14 text-center shadow-card">
        <ArrowDownToLine className="size-10 text-accent" aria-hidden />
        <p className="mt-4 font-display text-2xl font-bold">{t("drop.overlay")}</p>
        <p className="mt-1 text-sm text-muted">{t("drop.overlayHint")}</p>
      </div>
    </div>
  );
}
