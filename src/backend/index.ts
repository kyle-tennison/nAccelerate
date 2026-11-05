import type { Backend } from "../types";
import { PreviewBackend } from "./preview";
import { TauriBackend } from "./tauri";

export const inTauri = "__TAURI_INTERNALS__" in window;

// Outside Tauri (plain `npm run dev` in a browser) a simulated backend replays a real solve so the UI can be worked on.
export const backend: Backend = inTauri ? new TauriBackend() : new PreviewBackend();
