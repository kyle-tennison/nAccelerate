import { cancelJob, startJob } from "../app/job";
import { isActive, state } from "../app/state";
import { byId, setText } from "../lib/dom";
import { duration } from "../lib/format";

export function renderFooter() {
  const p = state.phase;
  const live = isActive();
  const run = state.run;
  const elapsed = run.startedAt ? (live ? Date.now() : run.endedAt) - run.startedAt : 0;
  setText("elapsed", run.startedAt ? `${duration(elapsed)} elapsed` : "");

  const go = byId<HTMLButtonElement>("go");
  const ready = !!state.folder?.notebook && !!state.settings.selfHosted.host.trim();
  go.classList.toggle("stop", live);
  go.innerHTML =
    p === "cancelling" ? `<i class="spin"></i>Cancelling` : live ? "Cancel" : p === "done" ? "Solve again" : "Send &amp; solve";
  go.disabled = p === "cancelling" || (!live && !ready);
}

export function setupFooter() {
  byId("go").addEventListener("click", () => {
    if (state.phase === "cancelling") return;
    if (isActive()) cancelJob();
    else void startJob();
  });
}
