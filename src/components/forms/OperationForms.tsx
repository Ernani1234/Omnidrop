import { useId } from "react";
import { type MessageKey, useT } from "../../i18n";
import type { ConversionCapability } from "../../lib/bindings/ConversionCapability";
import type { FileFormat } from "../../lib/bindings/FileFormat";
import type { OperationId } from "../../lib/bindings/OperationId";
import type { Preset } from "../../lib/bindings/Preset";
import { ARCHIVE_ORDER, ARCHIVE_PRESETS, FORMATS, QUALITY_PRESETS } from "../../lib/formats";
import type { OperationOptions } from "../../lib/operations";
import { useApp } from "../../store/app";
import { type Choice, ChoiceChips, Disclosure, Field, NativeSelect, RadioList, SectionLabel, Segmented, Switch, TextInput } from "../ui/primitives";

function useOptions<K extends keyof OperationOptions>(op: K) {
  const options = useApp((s) => s.options[op]);
  const setOptions = useApp((s) => s.setOptions);
  return [options, (patch: Partial<OperationOptions[K]>) => setOptions(op, patch)] as const;
}

function formatChoices(capability: ConversionCapability, order?: FileFormat[], unavailableLabel?: string): Choice<FileFormat>[] {
  const available = order ? order.filter((f) => capability.outputFormats.includes(f)) : capability.outputFormats;
  return [
    ...available.map((format) => ({ value: format, label: FORMATS[format].label })),
    ...capability.unavailableOutputFormats.map((format) => ({
      value: format,
      label: FORMATS[format].label,
      disabled: true,
      description: unavailableLabel,
    })),
  ];
}

function FormatPicker({ capability, value, onChange, order }: { capability: ConversionCapability; value: FileFormat; onChange: (f: FileFormat) => void; order?: FileFormat[] }) {
  const t = useT();
  const labelId = useId();
  const hint = FORMATS[value]?.hint;
  return (
    <div>
      <SectionLabel id={labelId}>{t("format.title")}</SectionLabel>
      <ChoiceChips label={t("format.title")} value={value} onChange={onChange} choices={formatChoices(capability, order, t("format.unavailable"))} />
      {hint && <p className="mt-2 text-[13px] text-muted">{t(hint)}</p>}
    </div>
  );
}

function PresetPicker({
  value,
  onChange,
  presets = QUALITY_PRESETS,
  labels,
  hints,
}: {
  value: Preset;
  onChange: (p: Preset) => void;
  presets?: Preset[];
  labels?: Partial<Record<Preset, MessageKey>>;
  hints?: Partial<Record<Preset, MessageKey>>;
}) {
  const t = useT();
  const choices = presets.map((preset) => ({ value: preset, label: t(labels?.[preset] ?? (`preset.${preset}` as MessageKey)) }));
  const hint = hints?.[value] ?? (`preset.${value}.hint` as MessageKey);
  return (
    <div>
      <SectionLabel>{t("preset.title")}</SectionLabel>
      <Segmented label={t("preset.title")} value={presets.includes(value) ? value : "balanced"} onChange={onChange} choices={choices} />
      <p className="mt-2 text-[13px] text-muted">{t(hint)}</p>
    </div>
  );
}

function PagesField({ mode, pages, onChange }: { mode: "all" | "some"; pages: string; onChange: (patch: { mode?: "all" | "some"; pages?: string }) => void }) {
  const t = useT();
  const id = useId();
  return (
    <div className="grid gap-2.5">
      <SectionLabel>{t("pdf.pagesLabel")}</SectionLabel>
      <RadioList
        label={t("pdf.pagesLabel")}
        value={mode}
        onChange={(m) => onChange({ mode: m })}
        choices={[
          { value: "all", label: t("pdf.pagesAll") },
          { value: "some", label: t("pdf.pagesSome") },
        ]}
      />
      {mode === "some" && (
        <Field label={t("pdf.pagesLabel")} hint={t("pdf.pagesHint")} htmlFor={id}>
          <TextInput id={id} value={pages} placeholder="1-3, 5" onChange={(e) => onChange({ pages: e.target.value })} />
        </Field>
      )}
    </div>
  );
}

function ImageForm({ capability }: { capability: ConversionCapability }) {
  const t = useT();
  const [o, set] = useOptions("imageConvert");
  const lossy = FORMATS[o.target].presets === "quality";
  const qualityId = useId();
  const widthId = useId();
  const heightId = useId();
  const percentId = useId();
  const colorId = useId();
  return (
    <div className="grid gap-5">
      <FormatPicker capability={capability} value={o.target} onChange={(target) => set({ target })} />
      {lossy ? (
        <PresetPicker value={o.preset} onChange={(preset) => set({ preset, customQuality: false })} />
      ) : (
        <p className="rounded-xl bg-surface-2 px-3 py-2.5 text-[13px] text-muted">{t("preset.lossless")}</p>
      )}
      <Disclosure title={t("advanced.title")}>
        <div className="grid gap-4">
          {lossy && (
            <>
              <Switch label={t("advanced.customQuality")} checked={o.customQuality} onChange={(customQuality) => set({ customQuality })} />
              {o.customQuality && (
                <Field label={`${t("advanced.quality")}: ${o.quality}`} htmlFor={qualityId}>
                  <input
                    id={qualityId}
                    type="range"
                    min={1}
                    max={100}
                    value={o.quality}
                    onChange={(e) => set({ quality: Number(e.target.value) })}
                    className="w-full accent-[var(--accent)]"
                  />
                </Field>
              )}
            </>
          )}
          {o.target === "webp" && (
            <Switch label={t("advanced.lossless")} hint={t("advanced.losslessHint")} checked={o.lossless} onChange={(lossless) => set({ lossless })} />
          )}
          <Field label={t("advanced.resize")}>
            <NativeSelect value={o.resizeMode} onChange={(e) => set({ resizeMode: e.target.value as typeof o.resizeMode })} aria-label={t("advanced.resize")}>
              <option value="none">{t("advanced.resize.none")}</option>
              <option value="fit">{t("advanced.resize.fit")}</option>
              <option value="percent">{t("advanced.resize.percent")}</option>
            </NativeSelect>
          </Field>
          {o.resizeMode === "fit" && (
            <div className="grid gap-2">
              <div className="grid grid-cols-2 gap-3">
                <Field label={t("advanced.maxWidth")} htmlFor={widthId}>
                  <TextInput id={widthId} inputMode="numeric" value={o.maxWidth} onChange={(e) => set({ maxWidth: e.target.value.replace(/\D/g, "") })} />
                </Field>
                <Field label={t("advanced.maxHeight")} htmlFor={heightId}>
                  <TextInput id={heightId} inputMode="numeric" value={o.maxHeight} onChange={(e) => set({ maxHeight: e.target.value.replace(/\D/g, "") })} />
                </Field>
              </div>
              <p className="text-xs text-muted">{t("advanced.fitHint")}</p>
            </div>
          )}
          {o.resizeMode === "percent" && (
            <Field label={`${t("advanced.percent")}: ${o.percent}%`} htmlFor={percentId}>
              <input
                id={percentId}
                type="range"
                min={5}
                max={200}
                step={5}
                value={o.percent}
                onChange={(e) => set({ percent: Number(e.target.value) })}
                className="w-full accent-[var(--accent)]"
              />
            </Field>
          )}
          {o.target === "jpeg" && (
            <Field label={t("advanced.background")} hint={t("advanced.backgroundHint")} htmlFor={colorId}>
              <input
                id={colorId}
                type="color"
                value={o.background}
                onChange={(e) => set({ background: e.target.value })}
                className="h-10 w-20 cursor-pointer rounded-lg border border-border-strong bg-surface p-1"
              />
            </Field>
          )}
        </div>
      </Disclosure>
    </div>
  );
}

function MergeForm() {
  const t = useT();
  const [o, set] = useOptions("pdfMerge");
  const id = useId();
  return (
    <div className="grid gap-4">
      <div className="rounded-xl bg-surface-2 px-3 py-2.5 text-[13px]">
        <p className="font-semibold">{t("pdf.order")}</p>
        <p className="text-muted">{t("pdf.orderHint")}</p>
      </div>
      <Field label={t("pdf.outputName")} htmlFor={id}>
        <TextInput id={id} value={o.outputName} placeholder={t("pdf.outputNameDefault")} onChange={(e) => set({ outputName: e.target.value })} />
      </Field>
    </div>
  );
}

function SplitForm() {
  const t = useT();
  const [o, set] = useOptions("pdfSplit");
  const id = useId();
  return (
    <div className="grid gap-3">
      <RadioList
        label={t("op.pdfSplit")}
        value={o.mode}
        onChange={(mode) => set({ mode })}
        choices={[
          { value: "each", label: t("pdf.split.each") },
          { value: "ranges", label: t("pdf.split.ranges") },
        ]}
      />
      {o.mode === "ranges" && (
        <Field label={t("pdf.rangesLabel")} hint={t("pdf.rangesHint")} htmlFor={id}>
          <TextInput id={id} value={o.ranges} placeholder="1-3, 4-6" onChange={(e) => set({ ranges: e.target.value })} />
        </Field>
      )}
    </div>
  );
}

function RotateForm() {
  const t = useT();
  const [o, set] = useOptions("pdfRotate");
  return (
    <div className="grid gap-5">
      <div>
        <SectionLabel>{t("pdf.angle")}</SectionLabel>
        <Segmented
          label={t("pdf.angle")}
          value={String(o.angle) as "90" | "180" | "270"}
          onChange={(angle) => set({ angle: Number(angle) as 90 | 180 | 270 })}
          choices={[
            { value: "90", label: t("pdf.angle.90") },
            { value: "180", label: t("pdf.angle.180") },
            { value: "270", label: t("pdf.angle.270") },
          ]}
        />
      </div>
      <PagesField mode={o.mode} pages={o.pages} onChange={(patch) => set(patch)} />
    </div>
  );
}

function PdfToImagesForm({ capability }: { capability: ConversionCapability }) {
  const [o, set] = useOptions("pdfToImages");
  return (
    <div className="grid gap-5">
      <FormatPicker capability={capability} value={o.target} onChange={(target) => set({ target })} />
      <PresetPicker
        value={o.preset}
        onChange={(preset) => set({ preset })}
        labels={{ smallest: "preset.dpi.smallest", balanced: "preset.dpi.balanced", quality: "preset.dpi.quality" }}
      />
      <PagesField mode={o.mode} pages={o.pages} onChange={(patch) => set(patch)} />
    </div>
  );
}

function AudioForm({ capability }: { capability: ConversionCapability }) {
  const t = useT();
  const [o, set] = useOptions("audioConvert");
  const target = capability.outputFormats.includes(o.target) ? o.target : capability.outputFormats[0];
  const lossy = target && FORMATS[target].presets === "quality";
  if (!target) return null;
  return (
    <div className="grid gap-5">
      <FormatPicker capability={capability} value={target} onChange={(next) => set({ target: next })} />
      {lossy ? (
        <PresetPicker value={o.preset} onChange={(preset) => set({ preset })} />
      ) : (
        <p className="rounded-xl bg-surface-2 px-3 py-2.5 text-[13px] text-muted">{t("preset.lossless")}</p>
      )}
    </div>
  );
}

const RESOLUTIONS = [2160, 1440, 1080, 720, 480];

function VideoForm({ capability }: { capability: ConversionCapability }) {
  const t = useT();
  const [o, set] = useOptions("videoConvert");
  const target = capability.outputFormats.includes(o.target) ? o.target : capability.outputFormats[0];
  if (!target) return null;
  return (
    <div className="grid gap-5">
      <FormatPicker capability={capability} value={target} onChange={(next) => set({ target: next })} />
      <PresetPicker value={o.preset} onChange={(preset) => set({ preset })} />
      <Disclosure title={t("advanced.title")}>
        <div className="grid gap-4">
          <Field label={t("advanced.maxResolution")}>
            <NativeSelect
              aria-label={t("advanced.maxResolution")}
              value={o.maxHeight ?? ""}
              onChange={(e) => set({ maxHeight: e.target.value ? Number(e.target.value) : null })}
            >
              <option value="">{t("advanced.resolution.original")}</option>
              {RESOLUTIONS.map((h) => (
                <option key={h} value={h}>
                  {h}p
                </option>
              ))}
            </NativeSelect>
          </Field>
          <Switch label={t("advanced.removeAudio")} checked={o.removeAudio} onChange={(removeAudio) => set({ removeAudio })} />
        </div>
      </Disclosure>
    </div>
  );
}

function ArchiveCreateForm({ capability, defaultName }: { capability: ConversionCapability; defaultName: string }) {
  const t = useT();
  const [o, set] = useOptions("archiveCreate");
  const id = useId();
  const compresses = FORMATS[o.target].presets === "archive";
  return (
    <div className="grid gap-5">
      <FormatPicker capability={capability} value={o.target} onChange={(target) => set({ target })} order={ARCHIVE_ORDER} />
      {compresses ? (
        <PresetPicker
          value={o.preset}
          onChange={(preset) => set({ preset })}
          presets={ARCHIVE_PRESETS}
          hints={{ smallest: "preset.archive.smallest.hint", balanced: "preset.archive.balanced.hint" }}
        />
      ) : (
        <p className="rounded-xl bg-surface-2 px-3 py-2.5 text-[13px] text-muted">{t("archive.noCompression")}</p>
      )}
      <Field label={t("archive.name")} htmlFor={id}>
        <TextInput id={id} value={o.outputName} placeholder={defaultName} onChange={(e) => set({ outputName: e.target.value })} />
      </Field>
    </div>
  );
}

export function OperationForm({ operation, capability, archiveName }: { operation: OperationId; capability: ConversionCapability; archiveName: string }) {
  const t = useT();
  switch (operation) {
    case "imageConvert":
      return <ImageForm capability={capability} />;
    case "pdfMerge":
      return <MergeForm />;
    case "pdfSplit":
      return <SplitForm />;
    case "pdfRotate":
      return <RotateForm />;
    case "pdfToImages":
      return <PdfToImagesForm capability={capability} />;
    case "audioConvert":
      return <AudioForm capability={capability} />;
    case "videoConvert":
      return <VideoForm capability={capability} />;
    case "archiveCreate":
      return <ArchiveCreateForm capability={capability} defaultName={archiveName} />;
    case "archiveExtract":
      return <p className="rounded-xl bg-surface-2 px-3 py-2.5 text-[13px] text-muted">{t("archive.extractInfo")}</p>;
  }
}
