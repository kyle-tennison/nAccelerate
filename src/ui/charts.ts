import { CORE_HISTORY } from "../app/state";
import type { Iteration } from "../lib/log";

const css = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

function fit(canvas: HTMLCanvasElement, h: number): [CanvasRenderingContext2D, number, number] {
  const dpr = window.devicePixelRatio || 1;
  const w = canvas.clientWidth;
  if (canvas.width !== Math.round(w * dpr) || canvas.height !== Math.round(h * dpr)) {
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
    canvas.style.height = `${h}px`;
  }
  const ctx = canvas.getContext("2d")!;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  return [ctx, w, h];
}

export function drawCores(canvas: HTMLCanvasElement, hist: number[][], n: number) {
  const cols = n <= 4 ? n : n <= 16 ? 4 : n <= 64 ? 8 : 16;
  const rows = Math.ceil(n / cols);
  const gap = cols >= 16 ? 2 : 3;
  const tileH = cols <= 4 ? 34 : cols <= 8 ? 24 : 18;
  const [ctx, w] = fit(canvas, rows * tileH + (rows - 1) * gap);
  const tw = (w - (cols - 1) * gap) / cols;
  const bg = css("--heat-0");
  const viz = css("--viz");
  ctx.clearRect(0, 0, w, canvas.height);
  for (let i = 0; i < n; i++) {
    const x = (i % cols) * (tw + gap);
    const y = Math.floor(i / cols) * (tileH + gap);
    ctx.fillStyle = bg;
    ctx.beginPath();
    ctx.roundRect(x, y, tw, tileH, 2);
    ctx.fill();
    const h = hist[i] || [];
    if (h.length < 2) continue;
    const px = (j: number) => x + ((j + CORE_HISTORY - h.length) / (CORE_HISTORY - 1)) * tw;
    const py = (v: number) => y + tileH - 1 - v * (tileH - 3);
    const trace = () => h.forEach((v, j) => (j ? ctx.lineTo(px(j), py(v)) : ctx.moveTo(px(j), py(v))));
    ctx.save();
    ctx.beginPath();
    ctx.roundRect(x, y, tw, tileH, 2);
    ctx.clip();
    ctx.beginPath();
    trace();
    ctx.lineTo(px(h.length - 1), y + tileH);
    ctx.lineTo(px(0), y + tileH);
    ctx.closePath();
    ctx.globalAlpha = 0.28;
    ctx.fillStyle = viz;
    ctx.fill();
    ctx.globalAlpha = 1;
    ctx.strokeStyle = viz;
    ctx.lineWidth = 1;
    ctx.beginPath();
    trace();
    ctx.stroke();
    ctx.restore();
  }
}

interface Line {
  key: "obj" | "chg";
  name: string;
  y: (d: Iteration) => number;
  value: (d: Iteration) => string;
  color: string;
  width: number;
}

/** Objective (log scale) and max change (linear) against time, with hover to tell the two lines apart. */
export class ObjectiveChart {
  private its: Iteration[] = [];
  private hover: { i: number; key: Line["key"] } | null = null;
  private geo: { x: (t: number) => number; lines: Line[]; top: number; height: number } | null = null;

  constructor(
    private canvas: HTMLCanvasElement,
    private tip: HTMLElement,
  ) {
    canvas.addEventListener("mousemove", (ev) => this.onMove(ev));
    canvas.addEventListener("mouseleave", () => {
      this.hover = null;
      this.tip.hidden = true;
      this.draw(this.its);
    });
  }

  draw(its: Iteration[]) {
    this.its = its;
    const [ctx, w, h] = fit(this.canvas, 76);
    ctx.clearRect(0, 0, w, h);
    if (its.length < 2) return;
    const padX = 4;
    const top = 6;
    const bottom = 18;
    const pw = w - padX * 2;
    const ph = h - top - bottom;
    const t0 = its[0].t;
    const t1 = its[its.length - 1].t;
    const logs = its.map((d) => Math.log10(d.obj));
    const lo = Math.min(...logs);
    const hi = Math.max(...logs);
    const span = Math.max(hi - lo, 1e-3);
    const oLo = lo - span * 0.1;
    const oHi = hi + span * 0.1;
    const cMax = Math.max(...its.map((d) => d.chg)) * 1.1 || 1;
    const x = (t: number) => padX + ((t - t0) / Math.max(1, t1 - t0)) * pw;
    const lines: Line[] = [
      {
        key: "chg",
        name: "Max change",
        y: (d) => top + (1 - d.chg / cMax) * ph,
        value: (d) => d.chg.toFixed(3),
        color: css("--ink-2"),
        width: 1.2,
      },
      {
        key: "obj",
        name: "Objective (log)",
        y: (d) => top + ((oHi - Math.log10(d.obj)) / (oHi - oLo)) * ph,
        value: (d) => d.obj.toPrecision(4),
        color: css("--viz"),
        width: 1.8,
      },
    ];
    this.geo = { x, lines, top, height: ph };
    const clampY = (v: number) => Math.min(top + ph, Math.max(top, v));

    ctx.strokeStyle = css("--line");
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(padX, top + ph + 0.5);
    ctx.lineTo(w - padX, top + ph + 0.5);
    ctx.stroke();
    ctx.save();
    ctx.beginPath();
    ctx.rect(0, top - 2, w, ph + 2);
    ctx.clip();
    const o = lines[1];
    ctx.beginPath();
    its.forEach((d, i) => (i ? ctx.lineTo(x(d.t), o.y(d)) : ctx.moveTo(x(d.t), o.y(d))));
    ctx.lineTo(x(t1), top + ph);
    ctx.lineTo(x(t0), top + ph);
    ctx.closePath();
    ctx.globalAlpha = this.hover && this.hover.key !== "obj" ? 0.06 : 0.16;
    ctx.fillStyle = o.color;
    ctx.fill();
    for (const L of lines) {
      const on = this.hover?.key === L.key;
      const off = this.hover && !on;
      ctx.globalAlpha = off ? 0.3 : 1;
      ctx.strokeStyle = L.color;
      ctx.lineWidth = L.width + (on ? 0.8 : 0);
      ctx.beginPath();
      its.forEach((d, i) => (i ? ctx.lineTo(x(d.t), L.y(d)) : ctx.moveTo(x(d.t), L.y(d))));
      ctx.stroke();
    }
    ctx.globalAlpha = 1;
    ctx.restore();

    if (this.hover) {
      const d = its[this.hover.i];
      const hx = x(d.t);
      ctx.strokeStyle = css("--line");
      ctx.setLineDash([2, 3]);
      ctx.beginPath();
      ctx.moveTo(hx + 0.5, top);
      ctx.lineTo(hx + 0.5, top + ph);
      ctx.stroke();
      ctx.setLineDash([]);
      for (const L of lines) {
        ctx.beginPath();
        ctx.arc(hx, clampY(L.y(d)), this.hover.key === L.key ? 3.5 : 2.5, 0, Math.PI * 2);
        ctx.fillStyle = L.color;
        ctx.fill();
      }
    } else {
      const e = its[its.length - 1];
      ctx.beginPath();
      ctx.arc(x(e.t), clampY(o.y(e)), 3, 0, Math.PI * 2);
      ctx.fillStyle = css("--viz-hot");
      ctx.fill();
    }
    ctx.font = "11px 'Figtree', system-ui, sans-serif";
    ctx.fillStyle = css("--ink-2");
    ctx.textAlign = "center";
    ctx.fillText("Time", w / 2, h - 3);
  }

  private onMove(ev: MouseEvent) {
    if (!this.geo || this.its.length < 2) return;
    const r = this.canvas.getBoundingClientRect();
    const mx = ev.clientX - r.left;
    const my = ev.clientY - r.top;
    let i = 0;
    let best = Infinity;
    this.its.forEach((d, j) => {
      const dx = Math.abs(this.geo!.x(d.t) - mx);
      if (dx < best) {
        best = dx;
        i = j;
      }
    });
    const d = this.its[i];
    const L = this.geo.lines.reduce((a, b) => (Math.abs(a.y(d) - my) <= Math.abs(b.y(d) - my) ? a : b));
    this.hover = { i, key: L.key };
    this.tip.hidden = false;
    this.tip.innerHTML = `<b>${L.name}</b> ${L.value(d)} · iter ${d.i}`;
    this.tip.style.left = `${Math.min(r.width - 80, Math.max(80, this.geo.x(d.t)))}px`;
    this.tip.style.top = `${Math.min(this.geo.top + this.geo.height, Math.max(this.geo.top, L.y(d)))}px`;
    this.draw(this.its);
  }
}
