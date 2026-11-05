import { DEFAULT_SETTINGS } from "../defaults";
import { clock } from "../lib/format";
import type { Backend, Block, FolderInfo, Handlers, HostInfo, JobRequest, Phase, Settings } from "../types";
import { SAMPLE_LOG } from "./sample-log";

const STORE_KEY = "naccelerate-preview-settings";
const TICK_MS = 700;
const UPLOAD_BYTES = 48_200_000;

// The preview has no notebook to read, so it treats the sample log's steps as a chain, with failed steps as dead ends.
function sampleBlocks(): Block[] {
  const blocks: Block[] = [];
  let prev: string | null = null;
  for (const line of SAMPLE_LOG) {
    const ok = line.match(/\]: (.+) complete \d+ms$/)?.[1];
    const failed = line.match(/\[E\]: ([^:]+): /)?.[1];
    const name = ok ?? failed;
    if (!name) continue;
    blocks.push({ name, deps: prev ? [prev] : [] });
    if (ok) prev = ok;
  }
  return blocks;
}

/** Simulates a server by replaying a real ntopcl log, for working on the UI in a browser. */
export class PreviewBackend implements Backend {
  private h: Handlers | null = null;
  private timer: number | undefined;
  private stopping = false;

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
    return { path: "/preview/bracket_topopt", name: "bracket_topopt", notebook: "bracket_topopt.ntop", steps: 178, blocks: sampleBlocks() };
  }

  async listen(h: Handlers) {
    this.h = h;
  }

  // Browsers don't expose dropped folder paths, so the preview treats any drop as the sample folder.
  onFolderDrop(hover: (over: boolean) => void, drop: (folder: FolderInfo) => void) {
    window.addEventListener("dragover", (e) => {
      e.preventDefault();
      hover(true);
    });
    window.addEventListener("dragleave", () => hover(false));
    window.addEventListener("drop", async (e) => {
      e.preventDefault();
      hover(false);
      drop(await this.pickFolder());
    });
  }

  startResize() {}
  minimize() {}
  close() {}

  async startJob(req: JobRequest) {
    let phase: Phase = "queued";
    let cursor = 0;
    let since = Date.now();
    let mem = 0;
    this.phase(phase);
    this.log("I", "Connecting to preview host");
    this.timer = window.setInterval(() => {
      const busy = phase === "solve" && !this.stopping;
      const line = SAMPLE_LOG[Math.min(cursor, SAMPLE_LOG.length - 1)] ?? "";
      const parallel = busy && /itr:|Static Structural|Mesh|FE Model/.test(line);
      mem += ((busy ? req.mem * 0.6 : 1) - mem) * 0.2;
      this.h?.stats({
        cores: Array.from({ length: req.cpu }, (_, i) =>
          Math.min(1, Math.max(0, parallel ? 0.9 + (Math.random() - 0.5) * 0.18 : i === 0 && busy ? 0.95 : 0.05 * Math.random())),
        ),
        memGb: mem,
        gpus: Array.from({ length: req.gpu }, () => ({ util: busy ? 0.6 + Math.random() * 0.3 : 0.01, vram: busy ? 0.63 : 0.03 })),
      });
      if (this.stopping) return;

      const elapsed = Date.now() - since;
      const next = (p: Phase) => {
        phase = p;
        since = Date.now();
        this.phase(p);
      };
      if (phase === "queued" && elapsed > 1500) {
        next("upload");
        this.log("I", `Uploading ${req.folder.split(/[\\/]/).pop()}/`);
      } else if (phase === "upload") {
        this.h?.progress({ done: Math.min(UPLOAD_BYTES, Math.round((elapsed / 3000) * UPLOAD_BYTES)), total: UPLOAD_BYTES });
        if (elapsed > 3000) next("solve");
      } else if (phase === "solve") {
        for (let n = 0; n < 4 && cursor < SAMPLE_LOG.length; n++) {
          const l = SAMPLE_LOG[cursor++];
          this.h?.log(l);
          if (/itr:/.test(l)) break;
        }
        if (cursor >= SAMPLE_LOG.length) {
          next("download");
          this.log("I", "Downloading bracket_topopt_solved.ntop");
        }
      } else if (phase === "download" && elapsed > 1500) {
        window.clearInterval(this.timer);
        this.log("I", "Received bracket_topopt_solved.ntop");
        this.log("I", "Disconnected from preview host");
        this.log("I", "Complete");
        this.phase("done");
      }
    }, TICK_MS);
  }

  async cancelJob() {
    this.stopping = true;
    this.phase("cancelling");
    this.log("I", "Stopping the remote solve");
    window.setTimeout(() => {
      window.clearInterval(this.timer);
      this.stopping = false;
      this.log("I", "Remote solve stopped");
      this.log("I", "Removed the remote job folder");
      this.log("W", "Cancelled");
      this.log("I", "Disconnected from preview host");
      this.phase("idle");
    }, 2500);
  }

  private phase(p: Phase) {
    this.h?.status(p);
  }

  private log(level: "I" | "W" | "E", text: string) {
    this.h?.log(`${clock()} [${level}]: ${text}`);
  }
}
