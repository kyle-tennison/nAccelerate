## Downloads

- Windows: `nAccelerate_<version>_x64-setup.exe`
- macOS (Apple Silicon and Intel): `nAccelerate_<version>_universal.dmg`

These builds aren't code-signed yet. On Windows, SmartScreen may warn you; click "More info", then "Run anyway". On macOS, the first launch is blocked; open System Settings, go to Privacy & Security, and click "Open Anyway" next to nAccelerate.

## Building from source

If you'd rather not run a downloaded installer, you can build this release yourself. You'll need [Node.js](https://nodejs.org) 20 or newer and [Rust](https://rustup.rs). On Linux, also install `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libsoup-3.0-dev` and `librsvg2-dev`.

Download the source code for this release from the assets below (or clone the repository and check out this release's tag), then run these from the project folder:

```bash
npm ci
```

```bash
npm run tauri build
```

The installer for your platform ends up in `src-tauri/target/release/bundle/`.
