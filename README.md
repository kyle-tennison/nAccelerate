# nAccelerate

A small desktop widget (Tauri) that sends an nTop project folder to a more powerful machine, runs the solve there, streams the log and the machine's CPU/memory/GPU usage, and brings the solved notebook back.

## How it works

1. Settings holds a self-hosted server (host, port, user, SSH key, remote job folder, solve command). AWS is not available yet.
2. Send & solve uploads the chosen folder with `scp` into a fresh job folder on the server.
3. The solve command runs over `ssh -tt` inside that job folder. The default is `taskset -c 0-{cpu_last} ntop --input {folder}`; `{folder}`, `{cpus}`, `{cpu_last}`, `{mem_gb}` and `{gpus}` are filled in, and `OMP_NUM_THREADS` and `CUDA_VISIBLE_DEVICES` are set from the chosen resources.
4. A second ssh connection streams `/proc/stat`, `/proc/meminfo` and `nvidia-smi` once a second for the CPU tiles, memory bar and GPU meters.
5. When the solver prints `Saved: <path>`, that file is downloaded next to the local notebook (as `<name>_solved.ntop`, or replacing it), the remote job folder is removed and the connection is closed.

The system OpenSSH client is used, so the server must accept key-based login (no password prompts). The server is assumed to be Linux.

## Development

```bash
npm install
```

```bash
npm run tauri dev
```

`npm run dev` alone serves the UI in a browser with a simulated backend that replays a real ntopcl log, which is handy for UI work.

On Linux, Tauri needs `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libsoup-3.0-dev` and `librsvg2-dev`.

## Not done yet

- AWS provider.
- The nTop login is stored in the system keychain but is not yet passed to the server.
- Memory is not enforced on the server unless the solve command uses `{mem_gb}`.
- Desktop notification when a solve finishes.
