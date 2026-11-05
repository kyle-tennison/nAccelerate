import { isActive, resetRun, state } from "../app/state";
import { backend } from "../backend";
import { byId, setText } from "../lib/dom";
import { render } from "./render";
import { resetSteps } from "./steps";

export function renderProject() {
  const f = state.folder;
  setText("fname", f ? `${f.name}/` : "No folder selected");
  setText("fsub", !f ? "Pick a folder with an .ntop file" : f.notebook || "No .ntop file in this folder");
}

export function setupProject() {
  byId("choose").addEventListener("click", async () => {
    const folder = await backend.pickFolder();
    if (!folder) return;
    state.folder = folder;
    if (!isActive()) {
      resetRun();
      resetSteps();
      state.phase = "idle";
    }
    render();
  });
}
