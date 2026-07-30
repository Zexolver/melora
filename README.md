# Melora

Melora is a native, lightweight, self-contained web browser being built by
assembling components salvaged from dormant and abandoned Rust
browser-engine projects, with a [Slint](https://slint.dev) chrome UI (tab
strip, address bar, navigation). No system WebView, no WebKit, no
external rendering dependency of any kind -- every tab's engine is
Melora's own code. The goal is a daily-driver browser that stays light
with hundreds of tabs open, by compressing tabs you aren't looking at
in RAM (LZ4, decompressed and re-parsed locally on demand -- no network
round-trip to restore one) instead of keeping every tab's engine resident
or discarding tabs outright.

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
- Tab compression: only the `N` most-recently-used tabs keep a live parsed
  document in memory; the rest are demoted to an LZ4-compressed copy of
  their source HTML in RAM and reconstructed locally on demand -- no
  network needed to restore a tab, unlike a browser that discards and
  re-fetches. Covered by tests including one that opens 300 tabs and
  checks both that only the budgeted few stay resident *and* that the
  total compressed footprint of the other 292 is far smaller than storing
  them uncompressed. See ARCHITECTURE.md for what this is and isn't
  (compresses source HTML and re-parses, not a live execution snapshot).
- Per-tab back/forward history.
- Real HTTP(S) fetching (`src/net.rs`, via `blitz-net`): typing an address,
  reload, back/forward, and (as a fallback only -- waking normally comes
  from the local compressed snapshot, not the network) reactivating a tab
  all genuinely fetch the page over the network, off the UI thread. See
  ARCHITECTURE.md for how it bridges tokio (which the fetch needs) to
  Slint's UI loop (which can't touch anything from another thread).
- A second engine candidate, [gosub-engine](https://github.com/gosub-io/gosub-engine)
  (MIT, actively developed), evaluated via a real dependency and test
  (`tests/gosub_html5_smoke.rs`) — its own from-scratch HTML5 parser
  verified working, though not wired into the tab pipeline yet.
- **Real pages actually render, styled, and scroll.** Fetched HTML is
  rasterized (CPU-only, via `blitz-paint` + `anyrender_vello_cpu`, no GPU
  surface needed) and shown in the content pane. Stylesheets, images, and
  fonts a page references are now fetched too (not just the top-level
  HTML), so pages look like the real site — real banners, real logos,
  real typography — not plain unstyled text. Mouse-wheel scrolling is
  wired through to the layout engine and repaints live.
- **Links work.** Clicking hit-tests the page, finds the nearest enclosing
  `<a href>`, resolves it against the page's URL, and navigates.
- **The window is actually responsive.** Resizing re-lays-out the page at
  the new size (confirmed live: a link's text visibly rewraps from two
  lines to one on narrowing), not just scaling a bitmap.
- **A real crash in the underlying engine, caught and contained.** Wiring
  up real stylesheets surfaced an actual bug in `blitz-dom` 0.1.4's table
  layout (a third-party bug, not fixable from here without patching that
  crate) that panicked on pypi.org's real page. `PageEngine` now catches
  panics from the engine's layout/paint calls so one page's rendering bug
  can't take the whole browser down — verified by literally re-running the
  exact steps that crashed it before the fix and confirming the process
  stays alive afterward. See ARCHITECTURE.md for the full story, including
  an earlier crash this same live-testing approach caught (a missing base
  URL that broke on any page with a relative link, i.e. almost every real
  page).

All of the above was checked against a real running instance under Xvfb
(driven with `xdotool`, screenshotted with `xwd`), not just unit-tested —
see ARCHITECTURE.md for the specific before/after evidence on each one.

What's not wired up yet — see the roadmap in ARCHITECTURE.md:

- JavaScript doesn't run, so anything client-side-rendered won't appear.
- Compressed tabs live in memory only; they don't survive a restart.
- The underlying `blitz-dom` table-layout bug above is contained, not
  fixed — affected pages stop laying out fully partway through rather
  than crashing, which is progress but not a real fix.

Platform target: Linux, Windows, and macOS, with no system rendering
dependency on any of them. iOS is deliberately out of scope, not dropped
quietly -- Apple requires all iOS browsers to render through WebKit, which
would mean compromising on "self-contained" specifically there; see
ARCHITECTURE.md.

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
