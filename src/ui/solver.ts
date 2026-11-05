import { isActive, state } from "../app/state";
import { byId, escapeHtml, setFill, setText } from "../lib/dom";
import { megabytes } from "../lib/format";
import { lineKind, optimizationResult, parseIterations } from "../lib/log";
import { ObjectiveChart } from "./charts";
import { render } from "./render";
import { renderSteps, stepsShown } from "./steps";

let objective: ObjectiveChart;

/** The main bar counts notebook steps (bytes while uploading or downloading); iterations get a second bar and a chart while they run. */
function renderProgress() {
  const p = state.phase;
  const run = state.run;
  const total = state.folder?.steps ?? 0;
  const steps = p === "solve" && total ? Math.min(total, stepsShown()) : null;
  const transfer = p === "upload" || p === "download" ? run.transfer : null;

  const bar = byId("prog");
  bar.classList.toggle("indet", isActive() && steps === null && !transfer);
  setFill(bar, p === "done" ? 1 : steps !== null ? steps / total : transfer ? transfer.done / Math.max(1, transfer.total) : 0);
  setText(
    "progTxt",
    steps !== null ? `Step ${steps}/${total}` : transfer ? `${megabytes(transfer.done)} / ${megabytes(transfer.total)} MB` : "",
  );

  const its = parseIterations(run.log);
  const last = its[its.length - 1];
  const iterating = !!last && !optimizationResult(run.log) && p === "solve";
  byId("iter-row").hidden = !iterating;
  if (iterating) setFill(byId("iter-prog"), last.i / last.max);
  setText("iterTxt", iterating ? `Iter ${last.i}/${last.max}` : "");
  byId("obj-wrap").hidden = !iterating || its.length < 2;
  if (iterating && its.length >= 2) objective.draw(its);
}

function renderLog() {
  const log = state.run.log;
  const errors = log.filter((l) => lineKind(l) === "err").length;
  const errs = byId("errs");
  errs.hidden = !errors;
  errs.textContent = `${errors} error${errors > 1 ? "s" : ""}`;

  const box = byId("log");
  box.hidden = !state.showAllLogs;
  byId("log-toggle").textContent = state.showAllLogs ? "Hide logs" : "Show all logs";
  if (!state.showAllLogs) return;
  const atBottom = box.scrollHeight - box.scrollTop - box.clientHeight < 24;
  box.innerHTML = log.map((l) => `<div class="l-${lineKind(l)}">${escapeHtml(l)}</div>`).join("");
  if (atBottom || !box.dataset.init) {
    box.scrollTop = box.scrollHeight;
    box.dataset.init = "1";
  }
}

export function renderSolver() {
  renderProgress();
  renderSteps();
  renderLog();
}

export function setupSolver() {
  objective = new ObjectiveChart(byId<HTMLCanvasElement>("obj"), byId("tip"));
  byId("log-toggle").addEventListener("click", () => {
    state.showAllLogs = !state.showAllLogs;
    delete byId("log").dataset.init;
    render();
  });
  byId("errs").addEventListener("click", () => {
    state.showAllLogs = true;
    render();
  });
}
