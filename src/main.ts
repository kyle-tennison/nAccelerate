import "@fontsource/ibm-plex-sans/400.css";
import "@fontsource/ibm-plex-sans/500.css";
import "@fontsource/ibm-plex-sans/600.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/archivo/800.css";
import { getBackend } from "./backend";
import { CORE_HISTORY, ObjectiveChart, drawCores } from "./charts";
import { countSteps, lineKind, optResult, parseIters, pretty } from "./logparse";
import { DEFAULT_SETTINGS } from "./mock";
import type { FolderInfo, Gpu, Phase, Progress, ResourceKey, Resources, Settings, Stats } from "./types";

const be = getBackend();
const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
const input = (id: string) => $<HTMLInputElement>(id);
const bind = (key: string, fn: (el: HTMLElement) => void) => document.querySelectorAll<HTMLElement>(`[data-bind="${key}"]`).forEach(fn);
const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;");

const RUNNING: Phase[] = ["queued", "upload", "solve", "download", "cancelling"];
const LABEL: Record<Phase, string> = { idle: "Ready", queued: "Queued", upload: "Uploading", solve: "Solving", download: "Downloading", cancelling: "Cancelling", done: "Complete" };
const MIN: Resources = { cpu: 1, mem: 1, gpu: 0 };
const KEYS: ResourceKey[] = ["cpu", "mem", "gpu"];

const S = {
  settings: structuredClone(DEFAULT_SETTINGS) as Settings,
  phase: "idle" as Phase,
  folder: null as FolderInfo | null,
  req: { cpu: 8, mem: 16, gpu: 0 } as Resources,
  coreHist: [] as number[][],
  memGb: 0,
  memPeak: 0,
  gpus: [] as Gpu[],
  log: [] as string[],
  upload: null as Progress | null,
  showAll: false,
  t0: 0,
  tEnd: 0,
};

const running = () => RUNNING.includes(S.phase);
const objective = new ObjectiveChart($<HTMLCanvasElement>("obj"), $("tip"));

function limits(): Resources | null {
  const h = S.settings.hostInfo;
  return h ? { cpu: h.cores, mem: Math.floor(h.memGb), gpu: h.gpus } : null;
}

function clampReq() {
  const L = limits();
  for (const k of KEYS) {
    let v = Math.round(S.req[k]) || MIN[k];
    if (L) v = Math.min(L[k], v);
    S.req[k] = Math.max(MIN[k], v);
  }
}

function fmt(ms: number) {
  const s = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), sec = String(s % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${sec}` : `${m}:${sec}`;
}

const mb = (bytes: number) => (bytes / 1e6).toFixed(bytes < 1e7 ? 1 : 0);

let frame = 0;
function schedule() {
  if (!frame) frame = requestAnimationFrame(() => { frame = 0; render(); });
}

function render() {
  clampReq();
  const p = S.phase, live = running();
  const host = S.settings.selfHosted.host.trim();
  bind("chip", (e) => { e.dataset.p = p; e.innerHTML = p === "cancelling" ? `<i class="spin"></i>${LABEL[p]}` : LABEL[p]; });
  bind("target", (e) => (e.textContent = host ? `Self-hosted (${host})` : "No server set up"));
  bind("fname", (e) => (e.textContent = S.folder ? `${S.folder.name}/` : "No folder selected"));
  bind("fsub", (e) => (e.textContent = !S.folder ? "Pick a folder with an .ntop file" : S.folder.notebook || "No .ntop file in this folder"));

  for (const k of KEYS) {
    document.querySelector(`[data-v="${k}"]`)!.innerHTML = k === "mem" ? `${S.req[k]}<small> GB</small>` : `${S.req[k]}`;
    input(`${k}-show`).disabled = live;
  }

  const n = S.coreHist.length || S.req.cpu;
  const cur = S.coreHist.length ? S.coreHist.reduce((a, h) => a + (h[h.length - 1] ?? 0), 0) / S.coreHist.length : 0;
  $("W").classList.toggle("offline", !live);
  bind("cpuTxt", (e) => (e.textContent = live ? `${n} cores · ${Math.round(cur * 100)}%` : S.coreHist.length ? "Disconnected" : "Not connected"));
  drawCores($<HTMLCanvasElement>("cores"), S.coreHist, n);

  const mm = $("mem-meter");
  (mm.firstElementChild as HTMLElement).style.width = `${Math.min(100, (S.memGb / S.req.mem) * 100)}%`;
  mm.classList.toggle("hot", S.memGb / S.req.mem > 0.85);
  bind("memTxt", (e) => (e.textContent = live ? `${S.memGb.toFixed(1)} GB` : S.memPeak ? `Peak ${S.memPeak.toFixed(1)} GB` : ""));

  $("gpu-sec").hidden = S.req.gpu === 0;
  const gp = $("gpus");
  if (gp.children.length !== S.req.gpu) {
    gp.innerHTML = Array.from({ length: S.req.gpu }, (_, i) => `<div class="gpu"><span class="dim">GPU ${i}</span><div class="meter mini"><i class="job"></i></div><div class="meter mini"><i class="job" style="background:var(--accent-2)"></i></div></div>`).join("");
  }
  [...gp.children].forEach((row, i) => {
    const g = S.gpus[i] ?? { util: 0, vram: 0 };
    (row.children[1].firstChild as HTMLElement).style.width = `${g.util * 100}%`;
    (row.children[2].firstChild as HTMLElement).style.width = `${g.vram * 100}%`;
  });

  // While the solver iterates, the bar runs 0 to the max iteration and the chart shows convergence. Once the iterating block finishes, both give way to plain step progress.
  const its = parseIters(S.log), lastIt = its[its.length - 1], finished = optResult(S.log);
  const iterating = !!lastIt && !finished && p === "solve";
  const up = p === "upload" || p === "download" ? S.upload : null;
  // The main bar counts notebook steps (upload and download use bytes instead); iterations get their own bar while they run.
  const total = S.folder?.steps ?? 0;
  const steps = p === "solve" && total ? Math.min(total, countSteps(S.log)) : null;
  const pr = $("prog");
  pr.classList.toggle("indet", live && steps === null && !up);
  (pr.firstChild as HTMLElement).style.width =
    p === "done" ? "100%" : steps !== null ? `${(steps / total) * 100}%` : up ? `${(up.done / Math.max(1, up.total)) * 100}%` : "0%";
  bind("progTxt", (e) => (e.textContent = steps !== null ? `Step ${steps}/${total}` : up ? `${mb(up.done)} / ${mb(up.total)} MB` : ""));
  $("iter-row").hidden = !iterating;
  if (iterating) (($("iter-prog").firstChild) as HTMLElement).style.width = `${(lastIt.i / lastIt.max) * 100}%`;
  bind("iterTxt", (e) => (e.textContent = iterating ? `Iter ${lastIt.i}/${lastIt.max}` : ""));
  $("obj-wrap").hidden = !iterating || its.length < 2;
  if (iterating && its.length >= 2) objective.draw(its);

  const last = S.log.length ? pretty(S.log[S.log.length - 1]) : { text: S.folder ? "Ready to send" : "Choose a folder to get started", kind: "" };
  $("latest").className = `msg trunc ${last.kind}`;
  bind("latest", (e) => (e.textContent = last.text));
  const nErr = S.log.filter((l) => / \[E\]: /.test(l)).length;
  $("errs").hidden = !nErr;
  $("errs").textContent = `${nErr} error${nErr > 1 ? "s" : ""}`;
  const elapsed = !S.t0 ? 0 : (live ? Date.now() : S.tEnd) - S.t0;
  bind("cost", (e) => (e.textContent = S.t0 ? `${fmt(elapsed)} elapsed` : ""));

  const go = $<HTMLButtonElement>("go");
  const cancelling = p === "cancelling";
  go.classList.toggle("stop", live);
  go.innerHTML = cancelling ? `<i class="spin"></i>Cancelling` : live ? "Cancel" : p === "done" ? "Solve again" : "Send &amp; solve";
  go.disabled = cancelling || (!live && (!S.folder?.notebook || !host));

  const box = $("log");
  box.hidden = !S.showAll;
  $("log-toggle").textContent = S.showAll ? "Hide logs" : "Show all logs";
  if (S.showAll) {
    const atBottom = box.scrollHeight - box.scrollTop - box.clientHeight < 24;
    box.innerHTML = S.log.map((l) => `<div class="l-${lineKind(l)}">${esc(l)}</div>`).join("");
    if (atBottom || !box.dataset.init) { box.scrollTop = box.scrollHeight; box.dataset.init = "1"; }
  }
}

function onStats(st: Stats) {
  if (S.coreHist.length !== st.cores.length) S.coreHist = st.cores.map(() => []);
  st.cores.forEach((v, i) => { const h = S.coreHist[i]; h.push(v); if (h.length > CORE_HISTORY) h.shift(); });
  S.memGb = st.memGb;
  S.memPeak = Math.max(S.memPeak, st.memGb);
  S.gpus = st.gpus;
  schedule();
}

function onStatus(p: Phase) {
  if (p !== S.phase) S.upload = null;
  S.phase = p;
  if (!RUNNING.includes(p)) S.tEnd = Date.now();
  schedule();
}

function onLog(line: string) {
  S.log.push(line);
  if (S.log.length > 20000) S.log.shift();
  schedule();
}

function localLog(level: "I" | "W" | "E", text: string) {
  onLog(`${new Date().toTimeString().slice(0, 8)} [${level}]: ${text}`);
}

async function start() {
  if (!S.folder?.notebook) return;
  S.log = []; S.coreHist = []; S.memGb = 0; S.memPeak = 0; S.gpus = []; S.upload = null;
  S.t0 = Date.now(); S.tEnd = 0; S.phase = "queued";
  S.settings.resources = { ...S.req };
  await be.saveSettings(S.settings);
  render();
  try {
    await be.startJob({ folder: S.folder.path, notebook: S.folder.notebook, ...S.req });
  } catch (e) {
    localLog("E", `nAccelerate: ${String(e)}`);
    S.phase = "idle"; S.tEnd = Date.now();
    render();
  }
}

$("go").addEventListener("click", () => {
  if (S.phase === "cancelling") return;
  if (running()) { S.phase = "cancelling"; render(); void be.cancelJob(); }
  else void start();
});
$("choose").addEventListener("click", async () => {
  const f = await be.pickFolder();
  if (!f) return;
  S.folder = f;
  if (!running()) { S.log = []; S.phase = "idle"; S.t0 = 0; }
  render();
});
$("log-toggle").addEventListener("click", () => { S.showAll = !S.showAll; delete $("log").dataset.init; render(); });
$("errs").addEventListener("click", () => { S.showAll = true; render(); });
$("min").addEventListener("click", () => be.minimize());
$("close").addEventListener("click", () => be.close());

document.querySelectorAll<HTMLElement>(".step").forEach((step) => {
  const k = step.dataset.k as ResourceKey;
  const show = step.querySelector<HTMLButtonElement>(".show")!, edit = step.querySelector<HTMLElement>(".edit")!;
  const field = edit.querySelector("input")!, hint = edit.querySelector<HTMLElement>(".max")!;
  const updateHint = () => {
    const L = limits(), v = Number(field.value);
    hint.textContent = L ? `Max ${L[k]}${k === "mem" ? " GB" : ""}` : "Test the connection to see limits";
    hint.classList.toggle("over", (L ? v > L[k] : false) || v < MIN[k]);
  };
  const close = (commit: boolean) => {
    if (edit.hidden) return;
    if (commit && field.value !== "") { S.req[k] = Number(field.value); if (S.phase === "done") S.phase = "idle"; }
    edit.hidden = true; show.hidden = false; render();
  };
  show.addEventListener("click", () => {
    field.value = String(S.req[k]); updateHint();
    show.hidden = true; edit.hidden = false; field.focus(); field.select();
  });
  field.addEventListener("input", updateHint);
  field.addEventListener("keydown", (e) => { if (e.key === "Enter") close(true); if (e.key === "Escape") close(false); });
  field.addEventListener("blur", () => close(true));
});

/* Settings save as they change; the password goes to the system keychain when its field loses focus. */

function formToSettings(): Settings {
  const s = structuredClone(S.settings);
  s.provider = "self";
  s.ntopUser = input("n-user").value.trim();
  s.selfHosted = {
    host: input("s-host").value.trim(),
    port: Number(input("s-port").value) || 22,
    user: input("s-user").value.trim(),
    keyPath: input("s-key").value.trim(),
    remoteDir: input("s-dir").value.trim() || DEFAULT_SETTINGS.selfHosted.remoteDir,
    command: input("s-cmd").value.trim() || DEFAULT_SETTINGS.selfHosted.command,
    cancelCommand: input("s-cancel").value.trim(),
  };
  s.results = { mode: $<HTMLSelectElement>("r-mode").value as "copy" | "replace", notify: s.results.notify, showAllLogs: input("r-logs").checked };
  return s;
}

let saveTimer: number | undefined;
async function persist() {
  window.clearTimeout(saveTimer);
  const before = S.settings.selfHosted.host;
  S.settings = formToSettings();
  if (S.settings.selfHosted.host !== before) S.settings.hostInfo = null;
  S.showAll = S.settings.results.showAllLogs;
  await be.saveSettings(S.settings);
}
function persistSoon() {
  window.clearTimeout(saveTimer);
  saveTimer = window.setTimeout(() => void persist(), 400);
}

async function fillForm() {
  const s = S.settings;
  input("prov-self").checked = true;
  input("n-user").value = s.ntopUser;
  input("n-pass").value = "";
  input("n-pass").placeholder = s.ntopUser && (await be.hasPassword(s.ntopUser)) ? "Saved" : "";
  input("s-host").value = s.selfHosted.host;
  input("s-port").value = String(s.selfHosted.port);
  input("s-user").value = s.selfHosted.user;
  input("s-key").value = s.selfHosted.keyPath;
  input("s-dir").value = s.selfHosted.remoteDir;
  input("s-cmd").value = s.selfHosted.command;
  input("s-cancel").value = s.selfHosted.cancelCommand ?? "";
  $<HTMLSelectElement>("r-mode").value = s.results.mode;
  input("r-logs").checked = s.results.showAllLogs;
  $("test-out").hidden = true;
  $("signin-out").hidden = true;
}

async function showSettings(on: boolean) {
  if (!on) await persist();
  $("v-main").hidden = on; $("v-set").hidden = !on;
  $("head-logo").hidden = on; $("head-title").hidden = !on;
  $("head-chip").hidden = on; $("back").hidden = !on; $("gear").hidden = on;
  if (on) void fillForm();
  else render();
}

function status(id: string, ok: boolean, title: string, lines: string[] = []) {
  const el = $(id);
  el.hidden = false;
  el.classList.toggle("bad", !ok);
  el.innerHTML = `<b>${esc(title)}</b>${lines.map((l) => `<span>${esc(l)}</span>`).join("")}`;
}

$("gear").addEventListener("click", () => void showSettings(true));
$("back").addEventListener("click", () => void showSettings(false));
document.querySelectorAll<HTMLInputElement | HTMLSelectElement>("#v-set input:not(#n-pass), #v-set select").forEach((el) => {
  el.addEventListener("input", persistSoon);
  el.addEventListener("change", persistSoon);
});
$("test").addEventListener("click", async () => {
  const btn = $<HTMLButtonElement>("test");
  btn.disabled = true; btn.textContent = "Testing…";
  try {
    await persist();
    const sh = S.settings.selfHosted;
    const h = await be.testConnection(S.settings);
    const hadLimits = !!limits();
    S.settings.hostInfo = h;
    if (!hadLimits && !S.settings.resources) S.req = { cpu: h.cores, mem: Math.floor(h.memGb), gpu: 0 };
    await be.saveSettings(S.settings);
    status("test-out", true, `Connected to ${sh.user ? `${sh.user}@${sh.host}` : sh.host}`, [
      `${h.os}${h.ntopFound ? " · solver found" : " · solver command not found"}`,
      `${h.cores} cores · ${h.memGb.toFixed(0)} GB · ${h.gpus ? `${h.gpus} GPU${h.gpus > 1 ? "s" : ""}` : "no GPU"}`,
    ]);
  } catch (e) {
    status("test-out", false, "Could not connect", [String(e)]);
  } finally {
    btn.disabled = false; btn.textContent = "Test connection";
  }
});
$("n-pass").addEventListener("change", async () => {
  const user = input("n-user").value.trim(), pass = input("n-pass").value;
  if (!pass) return;
  if (!user) { status("signin-out", false, "Enter your username first"); return; }
  try {
    await be.savePassword(user, pass);
    input("n-pass").value = "";
    input("n-pass").placeholder = "Saved";
    $("signin-out").hidden = true;
  } catch (e) {
    status("signin-out", false, "Could not save the password", [String(e)]);
  }
});

async function init() {
  await be.listen({ status: onStatus, log: onLog, stats: onStats, progress: (pr) => { S.upload = pr; schedule(); }, error: () => schedule() });
  S.settings = await be.loadSettings();
  S.showAll = S.settings.results.showAllLogs;
  const L = limits();
  S.req = S.settings.resources ?? (L ? { cpu: L.cpu, mem: L.mem, gpu: 0 } : { ...S.req });
  render();
  window.setInterval(() => { if (running()) render(); }, 1000);
  window.addEventListener("resize", schedule);
}

void init();
