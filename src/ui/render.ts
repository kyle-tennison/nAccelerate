import { clampResources } from "../app/state";
import { renderFooter } from "./footer";
import { renderHeader } from "./header";
import { renderMonitor } from "./monitor";
import { renderProject } from "./project";
import { renderResources } from "./resources";
import { renderSolver } from "./solver";

let frame = 0;

/** Redraws every section of the main view from the current state. */
export function render() {
  clampResources();
  renderHeader();
  renderProject();
  renderResources();
  renderMonitor();
  renderSolver();
  renderFooter();
}

/** Coalesces bursts of updates into one render per animation frame. */
export function schedule() {
  if (!frame)
    frame = requestAnimationFrame(() => {
      frame = 0;
      render();
    });
}
