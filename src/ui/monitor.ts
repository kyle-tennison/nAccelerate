import { isActive, state } from "../app/state";
import { byId, setFill, setText } from "../lib/dom";
import { drawCores } from "./charts";

/** CPU, memory and GPU usage. They freeze and grey out once the job disconnects. */
export function renderMonitor() {
  const live = isActive();
  const run = state.run;
  const req = state.resources;
  byId("app").classList.toggle("offline", !live);

  const cores = run.coreHistory.length || req.cpu;
  const latest = run.coreHistory.map((h) => h[h.length - 1] ?? 0);
  const load = latest.length ? latest.reduce((a, b) => a + b, 0) / latest.length : 0;
  setText("cpuTxt", live ? `${cores} cores · ${Math.round(load * 100)}%` : run.coreHistory.length ? "Disconnected" : "Not connected");
  drawCores(byId<HTMLCanvasElement>("cores"), run.coreHistory, cores);

  const meter = byId("mem-meter");
  const used = run.memGb / req.mem;
  setFill(meter, used);
  meter.classList.toggle("hot", used > 0.85);
  setText("memTxt", live ? `${run.memGb.toFixed(1)} GB` : run.memPeak ? `Peak ${run.memPeak.toFixed(1)} GB` : "");

  byId("gpu-sec").hidden = req.gpu === 0;
  const list = byId("gpus");
  if (list.childElementCount !== req.gpu) {
    list.innerHTML = Array.from(
      { length: req.gpu },
      (_, i) =>
        `<div class="gpu"><span class="dim">GPU ${i}</span><div class="meter mini"><i class="job"></i></div><div class="meter mini vram"><i class="job"></i></div></div>`,
    ).join("");
  }
  [...list.children].forEach((row, i) => {
    const g = run.gpus[i] ?? { util: 0, vram: 0 };
    setFill(row.children[1], g.util);
    setFill(row.children[2], g.vram);
  });
}
