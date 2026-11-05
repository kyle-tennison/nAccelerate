import { DEFAULT_SETTINGS } from "../defaults";
import type { FolderInfo, Gpu, Phase, Progress, ResourceKey, Resources, Settings } from "../types";

export const PHASE_LABEL: Record<Phase, string> = {
  idle: "Ready",
  queued: "Queued",
  upload: "Uploading",
  solve: "Solving",
  download: "Downloading",
  cancelling: "Cancelling",
  done: "Complete",
};

const ACTIVE: Phase[] = ["queued", "upload", "solve", "download", "cancelling"];

export const RESOURCE_KEYS: ResourceKey[] = ["cpu", "mem", "gpu"];
export const RESOURCE_MIN: Resources = { cpu: 1, mem: 1, gpu: 0 };

const LOG_LIMIT = 20000;
export const CORE_HISTORY = 42;

export const state = {
  settings: structuredClone(DEFAULT_SETTINGS) as Settings,
  phase: "idle" as Phase,
  folder: null as FolderInfo | null,
  resources: { cpu: 8, mem: 16, gpu: 0 } as Resources,
  showAllLogs: false,
  run: emptyRun(),
};

/** Everything collected while a job runs, reset when the next one starts. */
function emptyRun() {
  return {
    log: [] as string[],
    coreHistory: [] as number[][],
    memGb: 0,
    memPeak: 0,
    gpus: [] as Gpu[],
    transfer: null as Progress | null,
    startedAt: 0,
    endedAt: 0,
  };
}

export function resetRun() {
  state.run = emptyRun();
}

export function isActive(phase = state.phase) {
  return ACTIVE.includes(phase);
}

/** What the configured server has, once Test connection has measured it. */
export function serverLimits(): Resources | null {
  const h = state.settings.hostInfo;
  return h ? { cpu: h.cores, mem: Math.floor(h.memGb), gpu: h.gpus } : null;
}

export function clampResources() {
  const limits = serverLimits();
  for (const k of RESOURCE_KEYS) {
    let v = Math.round(state.resources[k]) || RESOURCE_MIN[k];
    if (limits) v = Math.min(limits[k], v);
    state.resources[k] = Math.max(RESOURCE_MIN[k], v);
  }
}

export function appendLog(line: string) {
  const log = state.run.log;
  log.push(line);
  if (log.length > LOG_LIMIT) log.shift();
}

export function recordStats(cores: number[], memGb: number, gpus: Gpu[]) {
  const run = state.run;
  if (run.coreHistory.length !== cores.length) run.coreHistory = cores.map(() => []);
  cores.forEach((v, i) => {
    const h = run.coreHistory[i];
    h.push(v);
    if (h.length > CORE_HISTORY) h.shift();
  });
  run.memGb = memGb;
  run.memPeak = Math.max(run.memPeak, memGb);
  run.gpus = gpus;
}
