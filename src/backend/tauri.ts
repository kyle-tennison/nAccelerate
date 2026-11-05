import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import type { Backend, FolderInfo, Handlers, HostInfo, JobRequest, Phase, Progress, ResizeDirection, Settings, Stats } from "../types";

export class TauriBackend implements Backend {
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
    return typeof path === "string" ? invoke<FolderInfo>("inspect_folder", { path }) : null;
  }

  onFolderDrop(hover: (over: boolean) => void, drop: (folder: FolderInfo) => void) {
    void getCurrentWebview().onDragDropEvent(async (e) => {
      const p = e.payload;
      if (p.type === "enter" || p.type === "over") hover(true);
      else if (p.type === "leave") hover(false);
      else if (p.type === "drop") {
        hover(false);
        if (p.paths.length) drop(await invoke<FolderInfo>("inspect_folder", { path: p.paths[0] }));
      }
    });
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
    await listen<Progress>("job-progress", (e) => h.progress(e.payload));
    await listen<string>("job-error", (e) => h.error(e.payload));
  }

  startResize(direction: ResizeDirection) {
    void getCurrentWindow().startResizeDragging(direction);
  }

  minimize() {
    void getCurrentWindow().minimize();
  }

  close() {
    void getCurrentWindow().close();
  }
}
