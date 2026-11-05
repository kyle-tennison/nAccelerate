export interface Pretty {
  text: string;
  kind: "" | "ok" | "warn" | "err";
}

export interface Iteration {
  t: number;
  i: number;
  max: number;
  obj: number;
  chg: number;
}

const LINE = /^(\d\d:\d\d:\d\d) \[(\w)\]: (.*)$/;

// Turns a raw ntopcl line into the one-line summary, e.g. "17:28:57 [I]: Design Response List_0 complete 0ms" -> "Step: Design Response List_0 (0ms)".
export function pretty(raw: string): Pretty {
  const m = raw.match(LINE);
  if (!m) return { text: raw, kind: /^Saved:/.test(raw) ? "ok" : "" };
  const [, , lvl, body] = m;
  let r: RegExpMatchArray | null;
  if (lvl === "E") return { text: `Error: ${body}`, kind: "err" };
  if ((r = body.match(/^itr:\s*(\d+)\/(\d+), obj: ([^,\s]+),.*time: (\d+)ms/))) {
    const ms = Number(r[4]);
    return { text: `Iteration ${r[1]} · objective ${Number(r[3]).toPrecision(4)} · ${ms >= 1000 ? `${(ms / 1000).toFixed(1)}s` : `${ms}ms`}`, kind: "" };
  }
  if ((r = body.match(/^(.+?) complete (\d+ms)$/))) return { text: `Step: ${r[1]} (${r[2]})`, kind: "" };
  if ((r = body.match(/^(.+?) completed in: (\d+ms)$/))) return { text: `Step: ${r[1]} (${r[2]})`, kind: "" };
  return { text: body, kind: lvl === "W" ? "warn" : /^Received|^Complete|successfully built|^File saved/.test(body) ? "ok" : "" };
}

export function lineKind(raw: string): Pretty["kind"] {
  if (/ \[E\]: /.test(raw)) return "err";
  if (/ \[W\]: /.test(raw)) return "warn";
  if (/\]: (Received|Complete)|successfully built|^Saved:/.test(raw)) return "ok";
  return "";
}

export interface StepEvent {
  name: string;
  ms: number | null;
  error: string | null;
}

// A step is logged once, either as "<block> complete <n>ms" or as an error "[E]: <block>: <message>". Errors the app itself logs are not steps.
export function stepEvents(log: string[]): StepEvent[] {
  const out: StepEvent[] = [];
  for (const l of log) {
    let r = l.match(/^\d\d:\d\d:\d\d \[I\]: (.+) complete (\d+)ms$/);
    if (r) { out.push({ name: r[1], ms: +r[2], error: null }); continue; }
    r = l.match(/^\d\d:\d\d:\d\d \[E\]: ([^:]+): (.*)$/);
    if (r && r[1] !== "nAccelerate") out.push({ name: r[1], ms: null, error: r[2] });
  }
  return out;
}

export function parseIters(log: string[]): Iteration[] {
  const out: Iteration[] = [];
  let dayOffset = 0;
  let prev = -1;
  for (const l of log) {
    const r = l.match(/^(\d\d):(\d\d):(\d\d) \[I\]: itr:\s*(\d+)\/(\d+), obj: ([^,\s]+),.*max change: ([\d.]+)/);
    if (!r) continue;
    let t = +r[1] * 3600 + +r[2] * 60 + +r[3] + dayOffset;
    if (t < prev) { dayOffset += 86400; t += 86400; }
    prev = t;
    out.push({ t, i: +r[4], max: +r[5], obj: +r[6], chg: +r[7] });
  }
  return out;
}

// The block that was iterating is only named once it finishes: it is the first "complete" line after the last itr line.
export function optResult(log: string[]): { name: string; ms: number } | null {
  let last = -1;
  log.forEach((l, i) => { if (/\]: itr:/.test(l)) last = i; });
  if (last < 0) return null;
  for (let i = last + 1; i < log.length; i++) {
    const r = log[i].match(/\]: (.+?) complete (\d+)ms$/);
    if (r) return { name: r[1], ms: +r[2] };
  }
  return null;
}
