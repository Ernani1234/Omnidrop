import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { isTauri, setBackend } from "./lib/api";
import { useApp } from "./store/app";
import "./styles.css";

async function start() {
  // Outside the desktop shell (plain `vite` in a browser) a simulated backend
  // lets the UI be developed and tested. It is never part of release builds.
  const mock = import.meta.env.DEV && !isTauri() ? await import("./lib/mock") : null;
  if (mock) setBackend(mock.createMockBackend());
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
  if (mock) void mock.runDemo((request) => useApp.getState().startJob(request), (name) => useApp.getState().openDialog(name));
}

void start();
