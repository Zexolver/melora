# Melora

Melora is a native, lightweight web browser being built by assembling
components salvaged from dormant and abandoned Rust browser-engine projects,
with a [Slint](https://slint.dev) chrome UI (tab strip, address bar,
navigation). The goal is a daily-driver browser that stays light with
hundreds of tabs open, by aggressively hibernating tabs you aren't looking
at instead of keeping every tab's engine resident in memory.

See [ARCHITECTURE.md](./ARCHITECTURE.md) for the research behind these
choices and the current state of the project.

## Status

This is an early scaffold, not a daily driver yet. What works today:

- A real Slint-based browser chrome: tab strip, address bar, back/forward/
  reload, status bar — compiles and runs.
- A real HTML parse → style → layout pipeline (via [Blitz](https://github.com/DioxusLabs/blitz),
  which itself is built from `html5ever`, `cssparser`/`selectors`, Stylo, and
  Taffy — components with roots in Servo's dormant years) driving every tab.
- Tab hibernation: only the `N` most-recently-used tabs keep a live parsed
  document in memory; the rest are dropped down to a few strings (url,
  title, scroll position) and reconstructed on demand. Covered by tests
  including one that opens 300 tabs and checks only the budgeted few stay
  resident.
- Per-tab back/forward history.

What's not wired up yet — see the roadmap in ARCHITECTURE.md:

- Real network fetching (pages are currently a generated placeholder, not
  the actual site at the URL you type).
- Painting the laid-out page to pixels inside the content area (the pane
  currently shows layout stats as text, not the rendered page).

## Building

```sh
cargo build
cargo test
cargo run
```

Requires a Rust toolchain with the 2024 edition (`rustc` ≥ 1.85) and, on
Linux, the usual windowing libraries (X11/Wayland + `libxkbcommon`).

## License

GPL-3.0-only (see [LICENSE](./LICENSE)). Slint is used under its GPLv3
option, which requires the combined work to be GPLv3 — see
[Slint's licensing FAQ](https://github.com/slint-ui/slint/blob/master/FAQ.md)
for details.
