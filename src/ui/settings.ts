/* Settings save as they change; the password goes to the system keychain when its field loses focus. */

import { serverLimits, state } from "../app/state";
import { backend } from "../backend";
import { DEFAULT_SETTINGS } from "../defaults";
import { byId, escapeHtml, field } from "../lib/dom";
import type { HostInfo, Settings } from "../types";
import { render } from "./render";

const SAVE_DELAY_MS = 400;
let saveTimer: number | undefined;

function readForm(): Settings {
  const s = structuredClone(state.settings);
  const d = DEFAULT_SETTINGS.selfHosted;
  s.provider = "self";
  s.ntopUser = field("n-user").value.trim();
  s.selfHosted = {
    host: field("s-host").value.trim(),
    port: Number(field("s-port").value) || 22,
    user: field("s-user").value.trim(),
    keyPath: field("s-key").value.trim(),
    remoteDir: field("s-dir").value.trim() || d.remoteDir,
    command: field("s-cmd").value.trim() || d.command,
    cancelCommand: field("s-cancel").value.trim() || d.cancelCommand,
  };
  s.results = {
    ...s.results,
    mode: document.querySelector<HTMLInputElement>('input[name="r-mode"]:checked')?.value === "replace" ? "replace" : "copy",
    showAllLogs: field("r-logs").checked,
  };
  return s;
}

async function fillForm() {
  const s = state.settings;
  const sh = s.selfHosted;
  field("prov-self").checked = true;
  field("n-user").value = s.ntopUser;
  field("n-pass").value = "";
  field("n-pass").placeholder = s.ntopUser && (await backend.hasPassword(s.ntopUser)) ? "Saved" : "";
  field("s-host").value = sh.host;
  field("s-port").value = String(sh.port);
  field("s-user").value = sh.user;
  field("s-key").value = sh.keyPath;
  field("s-dir").value = sh.remoteDir;
  field("s-cmd").value = sh.command;
  field("s-cancel").value = sh.cancelCommand;
  byId<HTMLDetailsElement>("adv").open = false;
  document.querySelectorAll<HTMLInputElement>('input[name="r-mode"]').forEach((r) => (r.checked = r.value === s.results.mode));
  field("r-logs").checked = s.results.showAllLogs;
  byId("test-out").hidden = true;
  byId("signin-out").hidden = true;
}

async function save() {
  window.clearTimeout(saveTimer);
  const previousHost = state.settings.selfHosted.host;
  state.settings = readForm();
  if (state.settings.selfHosted.host !== previousHost) state.settings.hostInfo = null;
  state.showAllLogs = state.settings.results.showAllLogs;
  await backend.saveSettings(state.settings);
}

function saveSoon() {
  window.clearTimeout(saveTimer);
  saveTimer = window.setTimeout(() => void save(), SAVE_DELAY_MS);
}

function showStatus(id: string, ok: boolean, title: string, lines: string[] = []) {
  const el = byId(id);
  el.hidden = false;
  el.classList.toggle("bad", !ok);
  el.innerHTML = `<b>${escapeHtml(title)}</b>${lines.map((l) => `<span>${escapeHtml(l)}</span>`).join("")}`;
}

async function open(on: boolean) {
  if (!on) await save();
  byId("v-main").hidden = on;
  byId("v-set").hidden = !on;
  byId("head-logo").hidden = on;
  byId("head-title").hidden = !on;
  byId("head-chip").hidden = on;
  byId("back").hidden = !on;
  byId("gear").hidden = on;
  if (on) void fillForm();
  else render();
}

function describeServer(h: HostInfo) {
  const gpus = h.gpus ? `${h.gpus} GPU${h.gpus > 1 ? "s" : ""}` : "no GPU";
  return [
    `${h.os} · ${h.ntopFound ? "Python and Docker found" : "Python or Docker missing"}`,
    `${h.cores} cores · ${h.memGb.toFixed(0)} GB · ${gpus}`,
  ];
}

async function testConnection() {
  const button = byId<HTMLButtonElement>("test");
  button.disabled = true;
  button.textContent = "Testing…";
  try {
    await save();
    const sh = state.settings.selfHosted;
    const info = await backend.testConnection(state.settings);
    const firstTest = !serverLimits();
    state.settings.hostInfo = info;
    // The first time the server's size is known, default to using all of it.
    if (firstTest && !state.settings.resources) state.resources = { cpu: info.cores, mem: Math.floor(info.memGb), gpu: 0 };
    await backend.saveSettings(state.settings);
    showStatus("test-out", true, `Connected to ${sh.user ? `${sh.user}@${sh.host}` : sh.host}`, describeServer(info));
  } catch (e) {
    showStatus("test-out", false, "Could not connect", [String(e)]);
  } finally {
    button.disabled = false;
    button.textContent = "Test connection";
  }
}

async function savePassword() {
  const user = field("n-user").value.trim();
  const password = field("n-pass").value;
  if (!password) return;
  if (!user) return showStatus("signin-out", false, "Enter your username first");
  try {
    await backend.savePassword(user, password);
    field("n-pass").value = "";
    field("n-pass").placeholder = "Saved";
    byId("signin-out").hidden = true;
  } catch (e) {
    showStatus("signin-out", false, "Could not save the password", [String(e)]);
  }
}

function restoreAdvancedDefaults() {
  const d = DEFAULT_SETTINGS.selfHosted;
  field("s-dir").value = d.remoteDir;
  field("s-cmd").value = d.command;
  field("s-cancel").value = d.cancelCommand;
  saveSoon();
}

export function setupSettings() {
  byId("gear").addEventListener("click", () => void open(true));
  byId("back").addEventListener("click", () => void open(false));
  byId("test").addEventListener("click", () => void testConnection());
  byId("n-pass").addEventListener("change", () => void savePassword());
  byId("adv-reset").addEventListener("click", restoreAdvancedDefaults);
  document.querySelectorAll<HTMLElement>("#v-set input:not(#n-pass), #v-set select").forEach((el) => {
    el.addEventListener("input", saveSoon);
    el.addEventListener("change", saveSoon);
  });
}
