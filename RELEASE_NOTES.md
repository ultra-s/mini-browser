## Mini Browser — v{{VERSION}}

Self-contained: **nothing else to download**. Put all the files from this release in one folder and run.

### Run it
- **Windows:** double-click `mini-browser.exe` (keep all `win-*.dll` / other files in the same folder)
- **Linux:** `chmod +x mini-browser-run && ./mini-browser-run`
- **Android:** install `Mini.apk`

### Files in this release
- `mini-browser` / `mini-browser.exe` — desktop browser (Linux / Windows)
- `mini-agent` / `mini-agent.exe` — agent CLI that drives the browser on `127.0.0.1:9777`
- `libcef` + `chrome_*.pak` + `icudtl.dat` + `v8_context_snapshot` — bundled Chromium 154 runtime
  (Windows copies are prefixed `win-` so both platforms can live in one folder)
- `Mini.apk` — Android build

### Environment knobs
- `MINI_STEALTH=1` — throwaway profile, uniform UA, no referrer, nothing persisted
- `MINI_UA=desktop|mobile|stealth|<literal>` — custom user-agent profile
- `MINI_PROXY=socks5://127.0.0.1:9050` — route through Tor/proxy
