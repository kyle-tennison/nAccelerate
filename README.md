<img alt="nAccelerate" src="assets/logo.svg" height="64">

nAccelerate runs your nTop notebooks on a bigger computer. You pick a project folder, and it copies the folder to your server, solves the notebook there and saves the result next to the original. While it runs you can watch the server's CPU and memory use, follow each step of the notebook, and see the objective converge during a topology optimization.

AWS support is planned but not available yet, so for now you need your own server.

## What you need

On the server:

- Linux, reachable over SSH with a key (password logins aren't supported)
- Docker, with an nTop image pulled
- Python 3

You also need an nTop account, since every solve signs in to nTop on the server.

## Installing

There are no prebuilt downloads yet, so build the app from this repository. You'll need [Node.js](https://nodejs.org) and [Rust](https://rustup.rs). On Linux, also install `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libsoup-3.0-dev` and `librsvg2-dev`.

```bash
npm install
```

```bash
npm run tauri build
```

The installer ends up in `src-tauri/target/release/bundle/`.

## Setting up

Open settings with the gear icon in the title bar.

Enter your nTop username and password. The password is stored in your system's keychain and is only sent to your server when a solve starts.

Under Compute, fill in your server's host, port, user name and the path to your SSH private key (for example `~/.ssh/id_ed25519`, not the `.pub` file). Then press Test connection. If it works, you'll see the server's cores, memory and GPUs, and whether Python and Docker were found. Those numbers become the maximums you can request.

Settings save as you type.

## Running a solve

1. Click Choose and pick the folder that holds your notebook. The folder should contain one `.ntop` file. Older `_solved.ntop` copies in the same folder are fine and won't be uploaded.
2. Click the CPU, memory or GPU numbers to change how much of the server the solve can use.
3. Press Send & solve.

The bar under Solver shows the upload, then how many of the notebook's steps have finished, then the download. During a topology optimization a second bar shows the iteration count, with a chart of the objective and the largest design change per iteration. Show all logs opens the full ntopcl output, and any errors are counted next to it.

When the solve finishes, the solved notebook is saved as `<name>_solved.ntop` next to your original. If you'd rather overwrite the original, change "When the solve finishes" in settings.

Cancel stops the solve on the server, removes its files there and then disconnects. It can take up to ten seconds.

## Good to know

ntopcl takes your password as a command-line argument, so other users on the server can see it in the process list while a solve is running. Use a server you trust.

The first nTop image Docker lists is used. If your server has more than one, pick one by changing the solve command under Advanced in settings, for example `NTOP_IMAGE=ntop:5.12 python3 ~/.naccelerate/ntop-run.py --input {folder}`.

If a solve fails, its files stay on the server under `~/naccelerate/jobs` so you can look at what went wrong. You can change that folder, and the commands used to run and cancel a solve, under Advanced in settings.
