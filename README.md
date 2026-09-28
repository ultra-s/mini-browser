# Mini Browser (Rust + CEF)

A minimalist native browser shell in Rust, embedding up-to-date Chromium (CEF 154 / Chromium 154)
via the [`cef-rs`](https://github.com/tauri-apps/cef-rs) crate. Inspired by @rauchg's Mini browser.

## Layout
- `src/main.rs` — browser-process entry point
- `src/bin/mini-browser_helper.rs` — CEF helper (renderer/GPU) process
- `src/shared/mod.rs` — CEF init + message loop
- `src/shared/simple_app.rs` — window/browser creation (Views framework, Chrome runtime style)
- `src/shared/simple_handler/` — title change, lifecycle, load-error handling (per-platform)

## Build & Run (Linux / macOS / Windows)
```sh
# 1. One-time: install Rust (https://rustup.rs)

# 2. Export CEF binaries (downloads ~1GB of Chromium once, cached in ~/.local/share/cef)
cargo install --git https://github.com/tauri-apps/cef-rs export-cef-dir   # or build from the repo
export-cef-dir --force $HOME/.local/share/cef

# 3. Build
export CEF_PATH="$HOME/.local/share/cef"
export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:$CEF_PATH"     # Linux
cargo build --release

# 4. Run (Linux: fix SUID sandbox first, or build with --no-default-features)
sudo chown root:root target/release/mini-browser_helper
sudo chmod 4755 target/release/mini-browser_helper
./target/release/mini-browser --url=https://example.com
```

## Flags
- `--url=<url>` — initial URL (default: duckduckgo.com)
- `--use-alloy-style` — Alloy (windowless-chrome) runtime instead of Chrome UI style

## Next steps toward a full "Mini"
- Address bar / tab strip via `browser_view_create` with a `ToolbarDelegate` (alloy style) or custom native UI
- Agent embed: run a local agent CLI over ACP (Agent Client Protocol) and expose page-read + tab-open via an MCP server
- macOS: add app bundle + native Swift window controls
