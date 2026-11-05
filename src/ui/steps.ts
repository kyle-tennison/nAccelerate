/* Step feed: the step that is probably running stays fixed on top, and finished steps enter just below it,
   pushing older ones down into a fade. The log only reports a block when it finishes, so the running one is
   inferred from the notebook: the first block, in file order, whose inputs have all finished. When many steps
   finish at once they are revealed quickly one after another, and the list keeps sliding instead of jumping. */

import { isActive, state } from "../app/state";
import { bind, byId, replay } from "../lib/dom";
import { elapsedSeconds, stepTime } from "../lib/format";
import { pretty, stepEvents, type StepEvent } from "../lib/log";
import type { Block } from "../types";

const ROW_H = 20;
const KEEP_DONE = 5;
export const REVEAL_MS = 45;

const feed = {
  /** Step events revealed so far; lags the log while a burst is being played out. */
  shown: 0,
  /** Step events already turned into rows. */
  rendered: 0,
  active: null as string | null,
  activeSince: 0,
  cache: { logLength: -1, events: [] as StepEvent[] },
};

function events(): StepEvent[] {
  const log = state.run.log;
  if (feed.cache.logLength !== log.length) feed.cache = { logLength: log.length, events: stepEvents(log) };
  return feed.cache.events;
}

/** How many steps have been shown, which is what the step progress bar counts. */
export function stepsShown() {
  return feed.shown;
}

export function resetSteps() {
  feed.shown = 0;
  feed.rendered = 0;
  feed.active = null;
  byId("run-list").replaceChildren();
}

/** The first block whose inputs have all finished, skipping blocks downstream of a failure. */
function runningBlock(blocks: Block[], finished: StepEvent[]): string | null {
  const done = new Set(finished.filter((e) => !e.error).map((e) => e.name));
  const dead = new Set(finished.filter((e) => e.error).map((e) => e.name));
  const known = new Set(blocks.map((b) => b.name));
  for (let changed = true; changed; ) {
    changed = false;
    for (const b of blocks) {
      if (!dead.has(b.name) && b.deps.some((d) => dead.has(d))) {
        dead.add(b.name);
        changed = true;
      }
    }
  }
  const ready = blocks.find((b) => !done.has(b.name) && !dead.has(b.name) && b.deps.every((d) => done.has(d) || !known.has(d)));
  return ready?.name ?? null;
}

function finishedRow(e: StepEvent) {
  const el = document.createElement("div");
  el.className = `run-row done${e.error ? " err" : ""}`;
  el.title = e.error ?? "";
  el.innerHTML = `<i class="ic"></i><span class="nm trunc"></span><span class="t num"></span>`;
  el.querySelector(".nm")!.textContent = e.error ? `${e.name} failed` : e.name;
  el.querySelector(".t")!.textContent = e.error ? "" : stepTime(e.ms ?? 0);
  return el;
}

/** Lifts the list by the rows just added on top, then lets it ease back down, continuing any slide already in flight. */
function glide(list: HTMLElement, rows: number) {
  if (!rows) return;
  const offset = new DOMMatrixReadOnly(getComputedStyle(list).transform).m42;
  list.style.transition = "none";
  list.style.transform = `translateY(${offset - rows * ROW_H}px)`;
  void list.offsetHeight;
  list.style.transition = "";
  list.style.transform = "translateY(0)";
}

function renderFinished(list: HTMLElement, evs: StepEvent[]) {
  let added = 0;
  while (feed.rendered < feed.shown && feed.rendered < evs.length) {
    list.prepend(finishedRow(evs[feed.rendered++]));
    added++;
  }
  const rows = list.querySelectorAll(".run-row");
  for (let i = KEEP_DONE; i < rows.length; i++) rows[i].remove();
  glide(list, added);
}

function renderRunning(row: HTMLElement, active: string | null) {
  const name = row.querySelector<HTMLElement>(".nm")!;
  if (active !== feed.active) {
    feed.active = active;
    feed.activeSince = Date.now();
    if (active) {
      name.textContent = active;
      replay(name, "swap");
    }
  }
  row.hidden = !active;
  if (active) row.querySelector(".t")!.textContent = elapsedSeconds(Date.now() - feed.activeSince);
}

export function renderSteps() {
  const p = state.phase;
  const evs = events();
  const list = byId("run-list");
  const running = byId("run-now");
  const solving = p === "solve" || (p === "cancelling" && evs.length > 0);

  if (solving) {
    renderFinished(list, evs);
    const blocks = state.folder?.blocks ?? [];
    renderRunning(running, p === "solve" && blocks.length ? runningBlock(blocks, evs.slice(0, feed.shown)) : null);
  } else {
    list.replaceChildren();
    feed.rendered = 0;
    running.hidden = true;
  }
  const feedVisible = !running.hidden || list.childElementCount > 0;
  byId("run").hidden = !feedVisible;

  // Outside the solve, the latest log line is shown instead.
  const log = state.run.log;
  const line = log.length
    ? pretty(log[log.length - 1])
    : { text: state.folder ? "Ready to send" : "Choose a folder to get started", kind: "" };
  const latest = byId("latest");
  latest.parentElement!.hidden = feedVisible;
  latest.className = `msg trunc ${line.kind}`;
  bind("latest", (el) => (el.textContent = line.text));
}

/** Reveals the next step(s), faster when a burst has piled up. Called every REVEAL_MS. */
export function advanceSteps() {
  const evs = events();
  if (!isActive()) {
    feed.shown = evs.length;
    return;
  }
  const backlog = evs.length - feed.shown;
  if (backlog > 0) feed.shown += backlog > 12 ? Math.ceil(backlog / 6) : 1;
  renderSteps();
}
