import { isActive, RESOURCE_KEYS, RESOURCE_MIN, serverLimits, state } from "../app/state";
import { byId, setText } from "../lib/dom";
import type { ResourceKey } from "../types";
import { render } from "./render";

export function renderResources() {
  const host = state.settings.selfHosted.host.trim();
  setText("target", host ? `Self-hosted (${host})` : "No server set up");
  for (const k of RESOURCE_KEYS) {
    const v = state.resources[k];
    document.querySelector(`[data-v="${k}"]`)!.innerHTML = k === "mem" ? `${v}<small> GB</small>` : `${v}`;
    byId<HTMLButtonElement>(`${k}-show`).disabled = isActive();
  }
}

/** Each resource shows its value; clicking it swaps in a number field with the server's limit beside it. */
export function setupResourceEditors() {
  document.querySelectorAll<HTMLElement>(".res-item").forEach((item) => {
    const key = item.dataset.k as ResourceKey;
    const show = item.querySelector<HTMLButtonElement>(".show")!;
    const edit = item.querySelector<HTMLElement>(".edit")!;
    const input = edit.querySelector("input")!;
    const hint = edit.querySelector<HTMLElement>(".max")!;

    const updateHint = () => {
      const limits = serverLimits();
      const v = Number(input.value);
      hint.textContent = limits ? `Max ${limits[key]}${key === "mem" ? " GB" : ""}` : "Test the connection to see limits";
      hint.classList.toggle("over", (limits ? v > limits[key] : false) || v < RESOURCE_MIN[key]);
    };
    const close = (commit: boolean) => {
      if (edit.hidden) return;
      if (commit && input.value !== "") {
        state.resources[key] = Number(input.value);
        if (state.phase === "done") state.phase = "idle";
      }
      edit.hidden = true;
      show.hidden = false;
      render();
    };

    show.addEventListener("click", () => {
      input.value = String(state.resources[key]);
      updateHint();
      show.hidden = true;
      edit.hidden = false;
      input.focus();
      input.select();
    });
    input.addEventListener("input", updateHint);
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") close(true);
      if (e.key === "Escape") close(false);
    });
    input.addEventListener("blur", () => close(true));
  });
}
