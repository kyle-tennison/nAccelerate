/** A clock-style duration: 4:07 or 1:02:09. */
export function duration(ms: number) {
  const s = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = String(s % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${sec}` : `${m}:${sec}`;
}

/** A step time as ntopcl would report it: 812ms, 9.9s, then clock style past a minute. */
export function stepTime(ms: number) {
  return ms < 1000 ? `${ms}ms` : ms < 60000 ? `${(ms / 1000).toFixed(1)}s` : duration(ms);
}

/** A live counter that ticks in whole seconds: 12s, then clock style past a minute. */
export function elapsedSeconds(ms: number) {
  return ms < 60000 ? `${Math.floor(ms / 1000)}s` : duration(ms);
}

export function megabytes(bytes: number) {
  return (bytes / 1e6).toFixed(bytes < 1e7 ? 1 : 0);
}

export function clock(date = new Date()) {
  return date.toTimeString().slice(0, 8);
}
