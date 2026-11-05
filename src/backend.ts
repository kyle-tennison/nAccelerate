import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { MockBackend } from "./mock";
import type { Backend, FolderInfo, Handlers, HostInfo, JobRequest, Phase, Settings, Stats } from "./types";

class TauriBackend implements Backend {
  loadSettings() {
    return invoke<Settings>("load_settings");
  }
  saveSettings(settings: Settings) {
    return invoke<void>("save_settings", { settings });
  }
  testConnection(settings: Settings) {
    return invoke<HostInfo>("test_connection", { settings });
  }
  savePassword(user: string, password: string) {
    return invoke<void>("save_password", { user, password });
  }
  hasPassword(user: string) {
    return invoke<boolean>("has_password", { user });
  }
  async pickFolder() {
    const path = await open({ directory: true, multiple: false, title: "Choose an nTop project folder" });
    if (typeof path !== "string") return null;
    return invoke<FolderInfo>("inspect_folder", { path });
  }
  startJob(request: JobRequest) {
    return invoke<void>("start_job", { request });
  }
  cancelJob() {
    return invoke<void>("cancel_job");
  }
  async listen(h: Handlers) {
    await listen<Phase>("job-status", (e) => h.status(e.payload));
    await listen<string>("job-log", (e) => h.log(e.payload));
    await listen<Stats>("job-stats", (e) => h.stats(e.payload));
    await listen<string>("job-error", (e) => h.error(e.payload));
  }
  minimize() {
    void getCurrentWindow().minimize();
  }
  close() {
    void getCurrentWindow().close();
  }
}

// Outside Tauri (plain `npm run dev` in a browser) a simulated backend replays a real solve so the UI can be worked on.
export function getBackend(): Backend {
  return "__TAURI_INTERNALS__" in window ? new TauriBackend() : new MockBackend();
}
