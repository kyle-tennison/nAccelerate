import { isActive, resetRun, state } from "../app/state";
import { backend } from "../backend";
import { byId, setText } from "../lib/dom";
import type { FolderInfo } from "../types";
import { render } from "./render";
import { resetSteps } from "./steps";

export function renderProject() {
  const f = state.folder;
  setText("fname", f ? `${f.name}/` : "No folder selected");
  setText("fsub", !f ? "Drop a folder here or click Choose" : f.notebook || "No .ntop file in this folder");
}

function useFolder(folder: FolderInfo) {
  state.folder = folder;
  if (!isActive()) {
    resetRun();
    resetSteps();
    state.phase = "idle";
  }
  render();
}

export function setupProject() {
  byId("choose").addEventListener("click", async () => {
    const folder = await backend.pickFolder();
    if (folder) useFolder(folder);
  });
  const zone = byId("drop-zone");
  backend.onFolderDrop(
    (over) => zone.classList.toggle("over", over && !isActive()),
    (folder) => {
      if (!isActive()) useFolder(folder);
    },
  );
}
