import { AlertCircle, Copy } from "lucide-react";
import { useState } from "react";
import { type MessageKey, useT } from "../i18n";
import type { EngineError } from "../lib/bindings/EngineError";
import type { ErrorCode } from "../lib/bindings/ErrorCode";
import type { TaskNote } from "../lib/bindings/TaskNote";
import { cn } from "./ui/primitives";

export function errorTitleKey(code: ErrorCode): MessageKey {
  return `error.${code}` as MessageKey;
}

export function errorHintKey(code: ErrorCode): MessageKey {
  return `error.${code}.hint` as MessageKey;
}

export function useErrorText() {
  const t = useT();
  return (error: EngineError) => ({ title: t(errorTitleKey(error.code)), hint: t(errorHintKey(error.code)) });
}

export function noteText(t: ReturnType<typeof useT>, note: TaskNote): string {
  return note.code === "unsafeEntriesSkipped"
    ? t("notes.unsafeEntriesSkipped", { count: note.count })
    : t(`notes.${note.code}` as MessageKey);
}

/** Plain-language error with a suggested action and optional technical details. */
export function ErrorMessage({ error, compact, className }: { error: EngineError; compact?: boolean; className?: string }) {
  const t = useT();
  const [copied, setCopied] = useState(false);
  const title = t(errorTitleKey(error.code));
  const hint = t(errorHintKey(error.code));
  return (
    <div className={cn("text-sm", className)} role={compact ? undefined : "alert"}>
      <p className="flex items-start gap-1.5 font-medium text-danger">
        <AlertCircle className="mt-0.5 size-4 shrink-0" aria-hidden />
        <span>{title}</span>
      </p>
      {!compact && hint && <p className="mt-0.5 pl-[22px] text-muted">{hint}</p>}
      {!compact && error.detail && (
        <details className="mt-1 pl-[22px]">
          <summary className="cursor-pointer text-xs text-subtle hover:text-text">{t("error.technical")}</summary>
          <div className="mt-1 flex items-start gap-2">
            <code className="block max-h-28 flex-1 overflow-auto rounded-lg bg-surface-2 p-2 text-xs break-all whitespace-pre-wrap text-muted">
              {error.code}: {error.detail}
            </code>
            <button
              type="button"
              className="inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-xs text-muted hover:bg-surface-2 hover:text-text"
              onClick={() => {
                navigator.clipboard?.writeText(`${error.code}: ${error.detail}`).then(() => setCopied(true));
              }}
            >
              <Copy className="size-3.5" aria-hidden />
              {copied ? t("error.copied") : t("error.copy")}
            </button>
          </div>
        </details>
      )}
    </div>
  );
}
