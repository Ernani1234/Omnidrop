import { History, Settings2, ShieldCheck } from "lucide-react";
import { useT } from "../i18n";
import { useApp } from "../store/app";
import { LogoMark } from "./Logo";
import { IconButton, Tooltip } from "./ui/primitives";

export function Header() {
  const t = useT();
  const openDialog = useApp((s) => s.openDialog);
  return (
    <header className="flex h-14 shrink-0 items-center justify-between border-b border-border bg-surface/80 px-5">
      <div className="flex items-center gap-2.5">
        <LogoMark size={28} />
        <span className="font-display text-[19px] font-bold tracking-tight">{t("app.name")}</span>
      </div>
      <div className="flex items-center gap-1.5">
        <Tooltip label={t("header.localHint")}>
          <span
            tabIndex={0}
            className="mr-2 hidden items-center gap-1.5 rounded-full bg-success-soft px-3 py-1 text-xs font-semibold text-success sm:inline-flex"
          >
            <ShieldCheck className="size-3.5" aria-hidden />
            {t("header.local")}
          </span>
        </Tooltip>
        <IconButton label={t("header.history")} onClick={() => openDialog("history")}>
          <History className="size-[18px]" />
        </IconButton>
        <IconButton label={t("header.settings")} onClick={() => openDialog("settings")}>
          <Settings2 className="size-[18px]" />
        </IconButton>
      </div>
    </header>
  );
}
