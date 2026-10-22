import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { App } from "../App";
import { setBackend } from "../lib/api";
import type { JobEvent } from "../lib/bindings/JobEvent";
import { DEFAULT_OPTIONS } from "../lib/operations";
import { useApp } from "../store/app";
import { createFakeBackend, type FakeBackend, imageFile } from "./fakeBackend";

let backend: FakeBackend;

function resetStore() {
  useApp.setState({
    ready: false,
    files: [],
    selection: [],
    operation: null,
    options: DEFAULT_OPTIONS,
    job: null,
    dialog: null,
    preview: { plan: null, error: null, loading: false },
    startError: null,
    thumbnails: {},
  });
}

async function renderApp() {
  render(<App />);
  await screen.findByRole("banner");
}

async function addFiles(files = [imageFile("a.png"), imageFile("b.png"), imageFile("c.png")]) {
  backend.files.push(...files);
  await act(async () => backend.emit("omnidrop://files", { files, truncated: false }));
  return files;
}

beforeEach(() => {
  resetStore();
  backend = createFakeBackend();
  setBackend(backend);
});

describe("empty state", () => {
  it("centers the screen on dropping files, with privacy notice", async () => {
    await renderApp();
    expect(screen.getByRole("heading", { name: "Solte seus arquivos aqui para começar." })).toBeInTheDocument();
    expect(screen.getByText("Tudo acontece neste computador. Nada é enviado para a internet.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /Escolher arquivos/ }));
    expect(backend.calls.some((c) => c.command === "pick_files")).toBe(true);
  });

  it("explains that audio and video need FFmpeg", async () => {
    await renderApp();
    expect(screen.getByText("Requer FFmpeg")).toBeInTheDocument();
  });
});

describe("conversion flow", () => {
  it("shows the queue, compatible operations and a clear call to action", async () => {
    await renderApp();
    await addFiles();
    expect(await screen.findByRole("heading", { name: "O que você gostaria de fazer?" })).toBeInTheDocument();
    const queue = screen.getByRole("list", { name: "Arquivos" });
    expect(within(queue).getAllByRole("listitem")).toHaveLength(3);
    expect(screen.getByRole("radio", { name: /Converter imagens/ })).toBeChecked();
    expect(screen.queryByRole("radio", { name: /Mesclar PDFs/ })).not.toBeInTheDocument();
    const cta = await screen.findByRole("button", { name: "Converter 3 arquivos" });
    await waitFor(() => expect(cta).toBeEnabled());
  });

  it("lets the user pick a format with the keyboard", async () => {
    await renderApp();
    await addFiles();
    const webp = await screen.findByRole("radio", { name: "WebP" });
    expect(webp).toBeChecked();
    webp.focus();
    // Hold the key: Radix checks the radio that receives focus while an arrow key is down.
    await userEvent.keyboard("{ArrowRight>}");
    await waitFor(() => expect(screen.getByRole("radio", { name: "AVIF" })).toBeChecked());
    await userEvent.keyboard("{/ArrowRight}");
    expect(await screen.findByText("Os menores arquivos. Conversão mais lenta.")).toBeInTheDocument();
  });

  it("runs a job and reports progress and results", async () => {
    await renderApp();
    await addFiles();
    const cta = await screen.findByRole("button", { name: "Converter 3 arquivos" });
    await waitFor(() => expect(cta).toBeEnabled());
    await userEvent.click(cta);
    const start = backend.calls.find((c) => c.command === "start_job");
    expect(start?.args?.request).toMatchObject({ op: "imageConvert", target: "webp" });

    const send = (event: JobEvent) => act(async () => backend.emit("omnidrop://job", event));
    await send({ type: "jobStarted", jobId: "job-1", total: 3 });
    await send({ type: "taskStarted", jobId: "job-1", taskId: "t0" });
    expect(await screen.findByText("Convertendo 1 de 3 arquivos…")).toBeInTheDocument();
    const output = { id: "o1", name: "a.webp", displayPath: "/photos/a.webp", isDir: false, size: 400, renamed: false };
    await send({ type: "taskFinished", jobId: "job-1", taskId: "t0", outcome: { status: "succeeded", outputs: [output], notes: [] } });
    await send({ type: "taskFinished", jobId: "job-1", taskId: "t1", outcome: { status: "failed", error: { code: "unreadable", detail: "bad data" } } });
    expect(await screen.findByText("Convertendo 3 de 3 arquivos…")).toBeInTheDocument();
    await send({ type: "taskFinished", jobId: "job-1", taskId: "t2", outcome: { status: "succeeded", outputs: [{ ...output, id: "o2", name: "c.webp" }], notes: [] } });
    await send({
      type: "jobFinished",
      jobId: "job-1",
      summary: { succeeded: 2, failed: 1, skipped: 0, cancelled: 0, outputs: [output], durationMs: 10, inputBytes: 3000, outputBytes: 800 },
    });

    expect(await screen.findByText("2 de 3 concluídos. 1 não puderam ser processados.")).toBeInTheDocument();
    expect(screen.getByText("→ a.webp")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Tentar novamente os que falharam/ })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /Abrir pasta/ }));
    expect(backend.calls.some((c) => c.command === "open_job_folder")).toBe(true);
  });

  it("cancels a running job", async () => {
    await renderApp();
    await addFiles();
    const cta = await screen.findByRole("button", { name: "Converter 3 arquivos" });
    await waitFor(() => expect(cta).toBeEnabled());
    await userEvent.click(cta);
    await userEvent.click(await screen.findByRole("button", { name: "Cancelar" }));
    expect(backend.calls.some((c) => c.command === "cancel_job")).toBe(true);
    expect(await screen.findByText("Cancelando…")).toBeInTheDocument();
  });

  it("explains unreadable files in plain language", async () => {
    await renderApp();
    await addFiles([imageFile("ok.png"), imageFile("broken.png", { issue: { code: "unreadable", detail: "crc" } })]);
    expect(
      await screen.findByText("Não foi possível ler este arquivo. Ele pode estar corrompido ou usar um formato não suportado."),
    ).toBeInTheDocument();
  });

  it("removes files with the keyboard", async () => {
    await renderApp();
    await addFiles();
    const remove = await screen.findByRole("button", { name: "Remover a.png" });
    await userEvent.click(remove);
    expect(backend.calls.some((c) => c.command === "remove_files")).toBe(true);
    expect(screen.getAllByRole("listitem")).toHaveLength(2);
    const next = screen.getByRole("button", { name: "Remover b.png" });
    next.focus();
    await userEvent.keyboard("{Delete}");
    expect(screen.getAllByRole("listitem")).toHaveLength(1);
  });
});
