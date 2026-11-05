import { SAMPLE_LOG } from "./sample-log";
import type { Backend, FolderInfo, Handlers, HostInfo, JobRequest, Phase, Settings } from "./types";

const STORE_KEY = "naccelerate-preview-settings";
const TICK_MS = 700;

export const DEFAULT_SETTINGS: Settings = {
  provider: "self",
  ntopUser: "",
  selfHosted: { host: "", port: 22, user: "", keyPath: "", remoteDir: "~/naccelerate/jobs", command: "ntop --input {folder}", cancelCommand: "sg docker -c 'docker rm -f {job}'" },
  aws: { region: "us-east-2", profile: "default", spendCap: "$25.00", spot: false, terminate: true },
  results: { mode: "copy", notify: true, showAllLogs: false },
  hostInfo: null,
  resources: null,
};

function stamp() {
  return new Date().toTimeString().slice(0, 8);
}

export class MockBackend implements Backend {
  private h: Handlers | null = null;
  private timer: number | undefined;

  async loadSettings() {
    try {
      const raw = localStorage.getItem(STORE_KEY);
      if (raw) return { ...DEFAULT_SETTINGS, ...JSON.parse(raw) } as Settings;
    } catch {
      // Storage can be unavailable in some browser contexts; fall back to defaults.
    }
    return structuredClone(DEFAULT_SETTINGS);
  }
  async saveSettings(settings: Settings) {
    try {
      localStorage.setItem(STORE_KEY, JSON.stringify(settings));
    } catch {
      // See loadSettings.
    }
  }
  async testConnection(settings: Settings): Promise<HostInfo> {
    if (!settings.selfHosted.host.trim()) throw new Error("Enter a host first.");
    await new Promise((r) => setTimeout(r, 600));
    return { cores: 64, memGb: 251.4, gpus: 2, os: "Linux 6.8.0 (preview)", ntopFound: true };
  }
  async savePassword() {}
  async hasPassword() {
    return false;
  }
  async pickFolder(): Promise<FolderInfo> {
    return { path: "/preview/bracket_topopt", name: "bracket_topopt", notebook: "bracket_topopt.ntop", steps: 178 };
  }
  async listen(h: Handlers) {
    this.h = h;
  }
  minimize() {}
  close() {}

  private stopping = false;

  async cancelJob() {
    this.stopping = true;
    this.status("cancelling");
    this.log("I", "Stopping the remote solve");
    window.setTimeout(() => {
      window.clearInterval(this.timer);
      this.stopping = false;
      this.log("I", "Remote solve stopped");
      this.log("I", "Removed the remote job folder");
      this.log("W", "Cancelled");
      this.log("I", "Disconnected from preview host");
      this.status("idle");
    }, 2500);
  }

  async startJob(req: JobRequest) {
    let phase: Phase = "queued";
    let cursor = 0;
    let since = Date.now();
    let mem = 0;
    this.status(phase);
    this.log("I", "Connecting to preview host");
    this.timer = window.setInterval(() => {
      const line = SAMPLE_LOG[Math.min(cursor, SAMPLE_LOG.length - 1)] ?? "";
      const busy = phase === "solve" && !this.stopping;
      const parallel = busy && /itr:|Static Structural|Mesh|FE Model/.test(line);
      mem += ((busy ? req.mem * 0.6 : 1) - mem) * 0.2;
      this.h?.stats({
        cores: Array.from({ length: req.cpu }, (_, i) => Math.min(1, Math.max(0, parallel ? 0.9 + (Math.random() - 0.5) * 0.18 : i === 0 && busy ? 0.95 : 0.05 * Math.random()))),
        memGb: mem,
        gpus: Array.from({ length: req.gpu }, () => ({ util: busy ? 0.6 + Math.random() * 0.3 : 0.01, vram: busy ? 0.63 : 0.03 })),
      });
      const elapsed = Date.now() - since;
      if (this.stopping) return;
      if (phase === "queued" && elapsed > 1500) {
        phase = "upload"; since = Date.now(); this.status(phase);
        this.log("I", `Uploading ${req.folder.split(/[\\/]/).pop()}/`);
      } else if (phase === "upload") {
        const total = 48_200_000;
        this.h?.progress({ done: Math.min(total, Math.round((elapsed / 3000) * total)), total });
        if (elapsed > 3000) { phase = "solve"; since = Date.now(); this.status(phase); }
      } else if (phase === "solve") {
        for (let n = 0; n < 4 && cursor < SAMPLE_LOG.length; n++) {
          const l = SAMPLE_LOG[cursor++];
          this.h?.log(l);
          if (/itr:/.test(l)) break;
        }
        if (cursor >= SAMPLE_LOG.length) {
          phase = "download"; since = Date.now(); this.status(phase);
          this.log("I", "Downloading bracket_topopt_solved.ntop");
        }
      } else if (phase === "download" && elapsed > 1500) {
        window.clearInterval(this.timer);
        this.log("I", "Received bracket_topopt_solved.ntop");
        this.log("I", "Disconnected from preview host");
        this.log("I", "Complete");
        this.status("done");
      }
    }, TICK_MS);
  }

  private status(p: Phase) {
    this.h?.status(p);
  }
  private log(level: string, text: string) {
    this.h?.log(`${stamp()} [${level}]: ${text}`);
  }
}
