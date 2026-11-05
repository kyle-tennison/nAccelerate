export const byId = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;

export const field = (id: string) => byId<HTMLInputElement>(id);

/** Runs `fn` on every element marked `data-bind="key"`. */
export function bind(key: string, fn: (el: HTMLElement) => void) {
  document.querySelectorAll<HTMLElement>(`[data-bind="${key}"]`).forEach(fn);
}

export function setText(key: string, text: string) {
  bind(key, (el) => (el.textContent = text));
}

export function escapeHtml(s: string) {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;");
}

/** Sets the fill of a bar whose first child is the filled part. */
export function setFill(bar: Element, fraction: number) {
  (bar.firstElementChild as HTMLElement).style.width = `${Math.max(0, Math.min(1, fraction)) * 100}%`;
}

/** Restarts a CSS animation tied to `cls`. */
export function replay(el: HTMLElement, cls: string) {
  el.classList.remove(cls);
  void el.offsetWidth;
  el.classList.add(cls);
}
