# nAccelerate

A small desktop widget (Tauri) that sends an nTop project folder to a more powerful machine, runs the solve there, streams the log and the machine's CPU/memory/GPU usage, and brings the solved notebook back.

## How it works

1. Settings holds the nTop login (the password lives in the system keychain) and a self-hosted server: host, port, user, SSH key, remote job folder, solve command and cancel command. AWS is not available yet.
2. Send & solve uploads the chosen folder with `scp` into a fresh job folder on the server. Other `.ntop` files in the folder, such as earlier `_solved` copies, are left out because the solver expects exactly one notebook.
3. The solve command runs over `ssh` inside the job folder (default `ntop --input {folder}`). It receives `NTOP_EMAIL`, `NTOP_PASSWORD` (sent over stdin, not on the command line), `NACCEL_JOB`, `NACCEL_CPUS`, `NACCEL_CPUSET`, `NACCEL_MEM_GB` and `NACCEL_GPUS`. `{folder}`, `{job}`, `{cpus}`, `{cpu_last}`, `{mem_gb}` and `{gpus}` in the command are also filled in.
4. A second ssh connection streams `/proc/stat`, `/proc/meminfo` and `nvidia-smi` once a second for the CPU tiles, memory bar and GPU meters. The first `NACCEL_CPUS` cores are shown, so pin the solver to `NACCEL_CPUSET`.
5. When the solver prints `Saved: <path>`, that file is downloaded next to the local notebook (as `<name>_solved.ntop`, or replacing it), the remote job folder is removed and the connection is closed.
6. Cancel kills the remote shell's process group and then runs the cancel command (default `sg docker -c 'docker rm -f {job}'`), since closing ssh does not stop a Docker container.

The system OpenSSH client is used, so the server must accept key-based login (no password prompts). The server is assumed to be Linux.

## Server wrapper

[`server/ntop`](server/ntop) is a wrapper that runs `ntopcl` in Docker. Put it on the server's `PATH` and set `IMAGE`. It names the container after `NACCEL_JOB` and applies `--cpuset-cpus`, `--memory` and `--gpus` from the resources chosen in the widget. Note that `ntopcl` takes the password as `-w`, so it is visible in the server's process list while a solve runs.

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
- Desktop notification when a solve finishes.
