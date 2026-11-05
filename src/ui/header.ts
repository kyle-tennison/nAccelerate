import { PHASE_LABEL, state } from "../app/state";
import { backend, inTauri } from "../backend";
import { bind, byId } from "../lib/dom";
import type { ResizeDirection } from "../types";

export function renderHeader() {
  const p = state.phase;
  bind("chip", (el) => {
    el.dataset.p = p;
    el.innerHTML = p === "cancelling" ? `<i class="spin"></i>${PHASE_LABEL[p]}` : PHASE_LABEL[p];
  });
}

/** Window chrome: macOS keeps its native traffic lights; elsewhere the window draws its own buttons and resize grips. */
export function setupWindowChrome() {
  if (inTauri && /Mac/.test(navigator.userAgent)) document.documentElement.classList.add("mac");
  byId("min").addEventListener("click", () => backend.minimize());
  byId("close").addEventListener("click", () => backend.close());
  document.querySelectorAll<HTMLElement>(".grips [data-dir]").forEach((grip) =>
    grip.addEventListener("mousedown", (e) => {
      if (e.button !== 0) return;
      e.preventDefault();
      backend.startResize(grip.dataset.dir as ResizeDirection);
    }),
  );
}
