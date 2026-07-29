# Melora

Melora is a native, lightweight web browser being built by assembling
components salvaged from dormant and abandoned Rust browser-engine projects,
with a [Slint](https://slint.dev) chrome UI (tab strip, address bar,
navigation). The goal is a daily-driver browser that stays light with
hundreds of tabs open, by aggressively hibernating tabs you aren't looking
at instead of keeping every tab's engine resident in memory.

The candidate projects come from
[rusty-web-browsers-any](https://github.com/stars/Zexolver/lists/rusty-web-browsers-any),
a curated list of dead/hobby/incomplete Rust browsers and engines. See
[ARCHITECTURE.md](./ARCHITECTURE.md) for how each entry was evaluated, the
research behind these choices, and the current state of the project.

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
- Real HTTP(S) fetching (`src/net.rs`, via `blitz-net`): typing an address,
  reload, back/forward, and waking a hibernated tab all genuinely fetch the
  page over the network now, off the UI thread. See ARCHITECTURE.md for
  how it bridges tokio (which the fetch needs) to Slint's UI loop (which
  can't touch anything from another thread).
- A second engine candidate, [gosub-engine](https://github.com/gosub-io/gosub-engine)
  (MIT, actively developed), evaluated via a real dependency and test
  (`tests/gosub_html5_smoke.rs`) — its own from-scratch HTML5 parser
  verified working, though not wired into the tab pipeline yet.

What's not wired up yet — see the roadmap in ARCHITECTURE.md:

- Painting the laid-out page to pixels inside the content area (the pane
  currently shows layout stats as text, not the rendered page, even though
  it's now real fetched content being parsed and laid out).
- Sub-resources: images, external stylesheets, and fonts referenced from a
  page aren't fetched yet, only the top-level HTML document.

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
