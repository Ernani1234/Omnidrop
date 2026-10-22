import { describe, expect, it } from "vitest";
import { en } from "../i18n/en";
import { plural, resolveLocale, translate } from "../i18n";
import { ptBR } from "../i18n/pt-BR";
import { archiveDefaultName, buildRequest, DEFAULT_OPTIONS, operationsFor, withAvailableTargets } from "../lib/operations";
import { formatBytes, formatDuration } from "../lib/units";
import { fileTask, jobProgress, type JobState } from "../store/app";
import { CAPABILITIES, imageFile } from "./fakeBackend";

describe("i18n", () => {
  it("has the same keys and no empty messages in every language", () => {
    expect(Object.keys(en).sort()).toEqual(Object.keys(ptBR).sort());
    const allowedEmpty = new Set(["error.cancelled.hint"]);
    for (const [key, value] of [...Object.entries(ptBR), ...Object.entries(en)]) {
      if (!allowedEmpty.has(key)) expect(value, key).not.toBe("");
    }
  });

  it("uses the same placeholders in every language", () => {
    const placeholders = (text: string) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
    for (const key of Object.keys(ptBR) as Array<keyof typeof ptBR>) {
      expect(placeholders(en[key]), key).toEqual(placeholders(ptBR[key]));
    }
  });

  it("interpolates and picks singular forms", () => {
    expect(translate("pt-BR", "cta.files", { verb: "Converter", count: 30 })).toBe("Converter 30 arquivos");
    expect(translate("en", plural("cta.files", 1), { verb: "Convert", count: 1 })).toBe("Convert 1 file");
    expect(translate("pt-BR", "running.imageConvert", { current: 8, total: 30 })).toBe("Convertendo 8 de 30 arquivos…");
  });

  it("resolves automatic language from the system", () => {
    expect(resolveLocale("auto", "pt-PT")).toBe("pt-BR");
    expect(resolveLocale("auto", "de-DE")).toBe("en");
    expect(resolveLocale("en", "pt-BR")).toBe("en");
  });

  it("keeps the microcopy defined in the specification", () => {
    expect(ptBR["drop.title"]).toBe("Solte seus arquivos aqui para começar.");
    expect(ptBR["op.question"]).toBe("O que você gostaria de fazer?");
    expect(ptBR["done.success"]).toBe("Tudo pronto. Seus arquivos estão na pasta de saída.");
    expect(ptBR["error.unreadable"]).toBe(
      "Não foi possível ler este arquivo. Ele pode estar corrompido ou usar um formato não suportado.",
    );
    expect(ptBR["error.unsupportedConversion"]).toBe(
      "Ainda não oferecemos esta conversão. Veja as opções disponíveis para este arquivo.",
    );
  });
});

describe("requests", () => {
  const defaults = { mergeName: "Merged", archiveName: "Files" };

  it("builds image requests from simple and advanced options", () => {
    expect(buildRequest("imageConvert", DEFAULT_OPTIONS, defaults)).toEqual({
      op: "imageConvert",
      target: "webp",
      preset: "balanced",
      quality: null,
      lossless: false,
      resize: null,
      background: "#ffffff",
    });
    const options = {
      ...DEFAULT_OPTIONS,
      imageConvert: { ...DEFAULT_OPTIONS.imageConvert, target: "jpeg" as const, customQuality: true, quality: 55, lossless: true, resizeMode: "fit" as const, maxWidth: "800", maxHeight: "" },
    };
    expect(buildRequest("imageConvert", options, defaults)).toMatchObject({
      target: "jpeg",
      quality: 55,
      lossless: false,
      resize: { mode: "fit", maxWidth: 800, maxHeight: null },
    });
  });

  it("builds PDF and archive requests with sensible defaults", () => {
    expect(buildRequest("pdfMerge", DEFAULT_OPTIONS, defaults)).toEqual({ op: "pdfMerge", outputName: "Merged" });
    expect(buildRequest("pdfSplit", DEFAULT_OPTIONS, defaults)).toEqual({ op: "pdfSplit", ranges: null });
    const ranges = { ...DEFAULT_OPTIONS, pdfSplit: { mode: "ranges" as const, ranges: " 1-3, 5 " } };
    expect(buildRequest("pdfSplit", ranges, defaults)).toEqual({ op: "pdfSplit", ranges: "1-3, 5" });
    expect(buildRequest("pdfRotate", DEFAULT_OPTIONS, defaults)).toEqual({ op: "pdfRotate", angle: 90, pages: null });
    expect(buildRequest("archiveCreate", DEFAULT_OPTIONS, defaults)).toEqual({ op: "archiveCreate", target: "zip", preset: "balanced", outputName: "Files" });
    expect(buildRequest("archiveExtract", DEFAULT_OPTIONS, defaults)).toEqual({ op: "archiveExtract" });
  });

  it("replaces targets this computer cannot produce", () => {
    const caps = CAPABILITIES.map((c) => (c.id === "archiveCreate" ? { ...c, outputFormats: ["7z" as const] } : c));
    expect(withAvailableTargets(DEFAULT_OPTIONS, caps).archiveCreate.target).toBe("7z");
  });

  it("suggests archive names", () => {
    expect(archiveDefaultName([imageFile("trip.png")], "Files")).toBe("trip");
    const a = imageFile("a.png", { relativeDir: "Trip/day1" });
    const b = imageFile("b.png", { relativeDir: "Trip" });
    expect(archiveDefaultName([a, b], "Files")).toBe("Trip");
    expect(archiveDefaultName([a, imageFile("c.png")], "Files")).toBe("Files");
  });
});

describe("operation choices", () => {
  it("lists only operations that apply, leading with the dominant family", () => {
    const pdf = (name: string) => imageFile(name, { format: "pdf", family: "pdf", details: { kind: "pdf", pages: 2 } });
    const files = [imageFile("a.png"), pdf("x.pdf"), pdf("y.pdf"), pdf("z.pdf")];
    const ids = operationsFor(files, CAPABILITIES).map((c) => c.meta.id);
    expect(ids).toEqual(["pdfMerge", "pdfSplit", "imageConvert", "archiveCreate"]);
    // Merge needs two PDFs.
    expect(operationsFor([pdf("x.pdf")], CAPABILITIES).map((c) => c.meta.id)).toEqual(["pdfSplit", "archiveCreate"]);
  });

  it("does not offer conversions for unreadable files", () => {
    const broken = imageFile("bad.png", { issue: { code: "unreadable", detail: "" } });
    expect(operationsFor([broken], CAPABILITIES).map((c) => c.meta.id)).toEqual([]);
  });
});

describe("units", () => {
  it("formats sizes and durations", () => {
    expect(formatBytes(0, "en")).toBe("0 B");
    expect(formatBytes(1536, "en")).toBe("1.5 KB");
    expect(formatBytes(4_812_331, "pt-BR")).toBe("4,6 MB");
    expect(formatDuration(65_000)).toBe("1:05");
    expect(formatDuration(3_725_000)).toBe("1:02:05");
  });
});

describe("job state", () => {
  const job: JobState = {
    id: "j",
    operation: "imageConvert",
    request: { op: "archiveExtract" },
    fileIds: ["a", "b"],
    order: ["t1", "t2"],
    tasks: {
      t1: { id: "t1", fileIds: ["a"], label: "a", outputName: "a", status: "succeeded", progress: 1, outcome: null },
      t2: { id: "t2", fileIds: ["b"], label: "b", outputName: "b", status: "running", progress: 0.5, outcome: null },
    },
    skipped: [],
    status: "running",
    summary: null,
  };

  it("aggregates progress and maps files to tasks", () => {
    expect(jobProgress(job)).toBeCloseTo(0.75);
    expect(fileTask(job, "b")?.id).toBe("t2");
    expect(fileTask(job, "zzz")).toBeNull();
  });
});
