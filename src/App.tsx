import { Tooltip } from "radix-ui";
import { useEffect, useRef } from "react";
import { Dialogs } from "./components/Dialogs";
import { DragOverlay, EmptyState } from "./components/EmptyState";
import { FileQueue } from "./components/FileQueue";
import { Header } from "./components/Header";
import { OperationPanel } from "./components/OperationPanel";
import { RunBar } from "./components/RunBar";
import { plural, resolveLocale, translate, useT } from "./i18n";
import { api, EVENTS, isTauri, on } from "./lib/api";
import { OPERATION_BY_ID } from "./lib/operations";
import { percent } from "./lib/units";
import { jobProgress, useApp } from "./store/app";

export function App() {
  const ready = useApp((s) => s.ready);
  const hasFiles = useApp((s) => s.files.length > 0);
  useBackendEvents();
  useTheme();
  useShortcuts();
  useJobAnnouncements();

  if (!ready) return <div className="h-full bg-bg" aria-busy="true" />;

  return (
    <Tooltip.Provider>
      <div className="flex h-full flex-col">
        <Header />
        <main className="flex min-h-0 flex-1 flex-col">
          {hasFiles ? (
            <div className="grid min-h-0 flex-1 grid-cols-1 gap-4 p-4 md:grid-cols-[minmax(0,1fr)_minmax(360px,420px)] md:p-5">
              <FileQueue />
              <OperationPanel />
            </div>
          ) : (
            <EmptyState />
          )}
          <RunBar />
        </main>
        <DragOverlay />
        <Dialogs />
        <LiveRegion />
      </div>
    </Tooltip.Provider>
  );
}

function useBackendEvents() {
  useEffect(() => {
    const store = useApp.getState;
    const unsubscribers = [
      on(EVENTS.files, ({ files, truncated }) => {
        if (files.length === 0 && !truncated) return;
        store().addFiles(files, truncated);
        const locale = resolveLocale(store().settings.language);
        store().announce(translate(locale, plural("live.added", files.length), { count: files.length }));
      }),
      on(EVENTS.inspecting, (event) => store().setInspecting(event)),
      on(EVENTS.drag, ({ active }) => store().setDragActive(active)),
      on(EVENTS.job, (event) => {
        store().handleJobEvent(event);
        if (event.type === "jobFinished") void store().refreshHistory();
      }),
      on(EVENTS.tools, (payload) => store().setTools(payload)),
    ];
    // Subscribe before asking for the initial state so no event is missed.
    void Promise.all(unsubscribers).then(() => api.bootstrap().then((b) => store().hydrate(b)));
    return () => {
      unsubscribers.forEach((p) => void p.then((unlisten) => unlisten()));
    };
  }, []);
}

function useTheme() {
  const theme = useApp((s) => s.settings.theme);
  const language = useApp((s) => s.settings.language);
  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const dark = theme === "dark" || (theme === "system" && media.matches);
      document.documentElement.dataset.theme = dark ? "dark" : "light";
      // Keep the native title bar in step with the page.
      if (isTauri()) {
        void import("@tauri-apps/api/window")
          .then(({ getCurrentWindow }) => getCurrentWindow().setTheme(theme === "system" ? null : theme))
          .catch(() => undefined);
      }
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);
  useEffect(() => {
    document.documentElement.lang = resolveLocale(language);
  }, [language]);
}

function isTyping(target: EventTarget | null) {
  const element = target as HTMLElement | null;
  return !!element && (element.tagName === "INPUT" || element.tagName === "TEXTAREA" || element.tagName === "SELECT" || element.isContentEditable);
}

function useShortcuts() {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const mod = event.ctrlKey || event.metaKey;
      if (!mod) return;
      const key = event.key.toLowerCase();
      if (key === "o") {
        event.preventDefault();
        void (event.shiftKey ? api.pickFolder() : api.pickFiles());
      } else if (key === ",") {
        event.preventDefault();
        useApp.getState().openDialog("settings");
      } else if (key === "a" && !isTyping(event.target) && useApp.getState().files.length > 0) {
        event.preventDefault();
        const { files, setSelection } = useApp.getState();
        setSelection(files.map((f) => f.id));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}

/** Announces start, quarter milestones and the end of a job to screen readers. */
function useJobAnnouncements() {
  const t = useT();
  const job = useApp((s) => s.job);
  const announce = useApp((s) => s.announce);
  const last = useRef<{ id: string; step: number } | null>(null);
  useEffect(() => {
    if (!job) return;
    if (last.current?.id !== job.id) {
      last.current = { id: job.id, step: 0 };
      announce(t("live.started", { label: t(OPERATION_BY_ID[job.operation].title) }));
      return;
    }
    if (job.status === "finished") return;
    const step = Math.floor(percent(jobProgress(job)) / 25);
    if (step > last.current.step && step < 4) {
      last.current.step = step;
      announce(t("live.progress", { percent: step * 25 }));
    }
  }, [job, announce, t]);
}

function LiveRegion() {
  const announcement = useApp((s) => s.announcement);
  return (
    <div className="sr-only" aria-live="polite" aria-atomic="true">
      <span key={announcement.id}>{announcement.text}</span>
    </div>
  );
}
