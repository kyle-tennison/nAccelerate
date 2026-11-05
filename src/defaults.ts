import type { Settings } from "./types";

// Mirrors the defaults in src-tauri/src/settings.rs.
export const DEFAULT_SETTINGS: Settings = {
  provider: "self",
  ntopUser: "",
  selfHosted: {
    host: "",
    port: 22,
    user: "",
    keyPath: "",
    remoteDir: "~/naccelerate/jobs",
    command: "python3 ~/.naccelerate/ntop-run.py --input {folder}",
    cancelCommand: "docker rm -f {job} || sg docker -c 'docker rm -f {job}'",
  },
  aws: { region: "us-east-2", profile: "default", spendCap: "$25.00", spot: false, terminate: true },
  results: { mode: "copy", notify: true, showAllLogs: false },
  hostInfo: null,
  resources: null,
};
