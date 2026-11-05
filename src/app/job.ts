import { backend } from "../backend";
import { clock } from "../lib/format";
import type { Handlers, Phase } from "../types";
import { render, schedule } from "../ui/render";
import { resetSteps } from "../ui/steps";
import { appendLog, isActive, recordStats, resetRun, state } from "./state";

function setPhase(p: Phase) {
  if (p !== state.phase) state.run.transfer = null;
  state.phase = p;
  if (!isActive(p)) state.run.endedAt = Date.now();
}

/** Updates from the backend while a job runs. */
export const jobEvents: Handlers = {
  status: (p) => {
    setPhase(p);
    schedule();
  },
  log: (line) => {
    appendLog(line);
    schedule();
  },
  stats: (s) => {
    recordStats(s.cores, s.memGb, s.gpus);
    schedule();
  },
  progress: (p) => {
    state.run.transfer = p;
    schedule();
  },
  error: () => schedule(),
};

export async function startJob() {
  const folder = state.folder;
  if (!folder?.notebook) return;
  resetRun();
  resetSteps();
  state.run.startedAt = Date.now();
  state.phase = "queued";
  state.settings.resources = { ...state.resources };
  await backend.saveSettings(state.settings);
  render();
  try {
    await backend.startJob({ folder: folder.path, notebook: folder.notebook, ...state.resources });
  } catch (e) {
    appendLog(`${clock()} [E]: nAccelerate: ${String(e)}`);
    setPhase("idle");
    render();
  }
}

export function cancelJob() {
  setPhase("cancelling");
  render();
  void backend.cancelJob();
}
