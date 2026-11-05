# nAccelerate

A small desktop widget (Tauri) that sends an nTop project folder to a more powerful machine, runs the solve there, streams the log and the machine's CPU/memory/GPU usage, and brings the solved notebook back.

## How it works

1. Settings holds the nTop login (the password lives in the system keychain) and a self-hosted server: host, port, user, SSH key, remote job folder, solve command and cancel command. AWS is not available yet.
2. Send & solve streams the chosen folder to a fresh job folder on the server as a tar archive over ssh, so the upload has a real progress bar. Other `.ntop` files in the folder, such as earlier `_solved` copies, are left out because the solver expects exactly one notebook.
3. The solve command runs over `ssh` inside the job folder (default `ntop --input {folder}`). It receives `NTOP_EMAIL`, `NTOP_PASSWORD` (sent over stdin, not on the command line), `NACCEL_JOB`, `NACCEL_CPUS`, `NACCEL_CPUSET`, `NACCEL_MEM_GB` and `NACCEL_GPUS`. `{folder}`, `{job}`, `{cpus}`, `{cpu_last}`, `{mem_gb}` and `{gpus}` in the command are also filled in.
4. A second ssh connection streams `/proc/stat`, `/proc/meminfo` and `nvidia-smi` once a second for the CPU tiles, memory bar and GPU meters. The first `NACCEL_CPUS` cores are shown, so pin the solver to `NACCEL_CPUSET`.
5. When the solver prints `Saved: <path>`, that file is downloaded over ssh with a progress bar next to the local notebook (as `<name>_solved.ntop`, or replacing it), the remote job folder is removed and the connection is closed.
6. Cancel shows a Cancelling state while it terminates the remote process group (killing it after 10 s), runs the cancel command (default `sg docker -c 'docker rm -f {job}'`, since closing ssh does not stop a Docker container), confirms the processes are gone, removes the remote job folder and only then disconnects.

The system OpenSSH client is used, so the server must accept key-based login (no password prompts). The server is assumed to be Linux.

## Step count

When a folder is chosen, the app reads the notebook's block list straight from the `.ntop` file ([`notebook.rs`](src-tauri/src/notebook.rs)) to know how many steps the run should log and how the blocks depend on each other. The solver bar then advances by one for each `… complete Nms` or `[E]: <block>: …` line, and a second bar tracks iterations while an optimization runs. Under the bar, the step running now stays fixed on top, and finished steps enter just below it and slide down into a fade. The log only reports blocks when they finish, so the running one is inferred as the first block whose inputs have all finished; when many steps finish at once they are shown one after another. The count assumes every block succeeds; a failed block skips its dependents, so a run with errors finishes short of the total. The `.ntop` layout is undocumented, so if the file doesn't match what's expected the bar falls back to an indeterminate one.

To check the count for a notebook:

```bash
cd src-tauri && cargo run --example count_steps -- path/to/notebook.ntop
```

## Server runner

Before each job the app installs [`ntop-run.py`](src-tauri/resources/ntop-run.py) to `~/.naccelerate/` on the server, and the default solve command runs it. It needs Python 3 and Docker on the server (Test connection checks for both). It runs `ntopcl` in the nTop Docker image (found automatically, or set `NTOP_IMAGE`), uses `sg docker` when the shell isn't in the docker group yet, names the container after the job so cancel can remove it, and applies `--cpuset-cpus`, `--memory` and `--gpus` from the resources chosen in the widget. `ntopcl` takes the password as `-w`, so it is visible in the server's process list while a solve runs.

The remote job folder, solve command and cancel command can be changed under Advanced in Settings.

## Development

```bash
npm install
```

```bash
npm run tauri dev
```

`npm run dev` alone serves the UI in a browser with a simulated backend that replays a real ntopcl log, which is handy for UI work.

On Linux, Tauri needs `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libsoup-3.0-dev` and `librsvg2-dev`.

## Code layout

Frontend (`src/`):

- `main.ts` wires everything up.
- `app/state.ts` holds the app state; `app/job.ts` starts and cancels jobs and applies backend events to the state.
- `ui/` has one module per part of the window (header, project, resources, monitor, solver, step feed, footer, settings), each with a `render…` function and a `setup…` function for its event handlers. `ui/render.ts` redraws them all.
- `backend/` talks to Rust through Tauri, or replays a real log in the browser preview.
- `lib/` has DOM helpers, formatting and the ntopcl log parser.

Backend (`src-tauri/src/`):

- `commands.rs` is everything the UI can call; `lib.rs` only registers it.
- `job.rs` runs one job end to end, using `server.rs` (checks, probe, runner install), `transfer.rs` (upload and download with progress), `solver.rs` (the remote solve and stopping it) and `stats.rs` (CPU, memory and GPU usage).
- `events.rs` is every event sent to the UI; `cancel.rs` is how a job stops early; `remote.rs` wraps `ssh`.
- `notebook.rs` reads the step plan from a `.ntop` file, `settings.rs` stores settings and `credentials.rs` the keychain password.

`npm run check` type-checks and checks formatting; `npm run format` formats.

## Not done yet

- AWS provider.
- Desktop notification when a solve finishes.
