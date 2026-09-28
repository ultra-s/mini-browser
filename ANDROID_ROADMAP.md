# Android Goal & Roadmap

## Goal (committed)
Ship "Mini for Android" — the same minimalist, agent-embeddable browser — as an APK
you can sideload, then publish to F-Droid / Play.

**Why not CEF on Android?** CEF has no Android port. The correct architecture (and what
this repo is set up for) is one shared Rust core (`core/`) with two shells:
- Desktop shell: CEF (Chromium) — already built and running.
- Android shell: GeckoView (Firefox engine — full extensions/anti-tracking, Rust-based)
  with system WebView as a lighter fallback. mini-core talks to both via JNI.

## Milestones
- [x] M0 Desktop shell builds & runs (Rust + CEF, Windows/macOS/Linux)
- [x] M1 Desktop features: persistent profile, multi-window/popups, remote-control server (agent transport)
- [x] M2 Shared core: TabManager (tab state machine) + AgentCommand/AgentEvent protocol
- [ ] M3 Desktop tab strip UI driven by mini-core TabManager (CEF alloy style toolbar)
- [ ] M4 Agent connect: `mini-agent` CLI speaks AgentCommand/AgentEvent over the local server
- [ ] M5 Android scaffold: Gradle project + GeckoView activity rendering mini-core tab state
- [ ] M6 JNI bridge: mini-core compiled with cargo-ndk (`cdylib`), commands/events across the boundary
- [ ] M7 Agent embed on device: foreground service running the agent loop
- [ ] M8 APK release: signing, R8, Play/F-Droid metadata

## Definition of done (Android)
1. `./gradlew assembleRelease` produces an installable APK.
2. Tabs opened/activated/closed on Android go through mini-core (same code as desktop).
3. An agent can send `AgentCommand::Open` and receive page text — same protocol as desktop.
