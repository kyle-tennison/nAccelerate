export type ResizeDirection = "North" | "South" | "East" | "West" | "NorthEast" | "NorthWest" | "SouthEast" | "SouthWest";
export type Phase = "idle" | "queued" | "upload" | "solve" | "download" | "cancelling" | "done";
export type ResourceKey = "cpu" | "mem" | "gpu";
export type Resources = Record<ResourceKey, number>;

export interface HostInfo {
  cores: number;
  memGb: number;
  gpus: number;
  os: string;
  ntopFound: boolean;
}

export interface Settings {
  provider: "self" | "aws";
  ntopUser: string;
  selfHosted: { host: string; port: number; user: string; keyPath: string; remoteDir: string; command: string; cancelCommand: string };
  aws: { region: string; profile: string; spendCap: string; spot: boolean; terminate: boolean };
  results: { mode: "copy" | "replace"; notify: boolean; showAllLogs: boolean };
  hostInfo: HostInfo | null;
  resources: Resources | null;
}

export interface Gpu {
  util: number;
  vram: number;
}

export interface Stats {
  cores: number[];
  memGb: number;
  gpus: Gpu[];
}

export interface Progress {
  done: number;
  total: number;
}

export interface FolderInfo {
  path: string;
  name: string;
  notebook: string | null;
  steps: number | null;
  blocks: Block[];
}

export interface Block {
  name: string;
  deps: string[];
}

export interface JobRequest extends Resources {
  folder: string;
  notebook: string;
}

export interface Handlers {
  status(phase: Phase): void;
  log(line: string): void;
  stats(stats: Stats): void;
  progress(progress: Progress): void;
  error(message: string): void;
}

export interface Backend {
  loadSettings(): Promise<Settings>;
  saveSettings(settings: Settings): Promise<void>;
  testConnection(settings: Settings): Promise<HostInfo>;
  savePassword(user: string, password: string): Promise<void>;
  hasPassword(user: string): Promise<boolean>;
  pickFolder(): Promise<FolderInfo | null>;
  /** Calls `hover` while something is dragged over the window and `drop` with the folder that was dropped. */
  onFolderDrop(hover: (over: boolean) => void, drop: (folder: FolderInfo) => void): void;
  startJob(request: JobRequest): Promise<void>;
  cancelJob(): Promise<void>;
  listen(handlers: Handlers): Promise<void>;
  startResize(direction: ResizeDirection): void;
  minimize(): void;
  close(): void;
}
