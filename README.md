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
- Tab compression, in **three tiers**: only the `N` most-recently-used
  tabs keep a live parsed document in memory; the next tier is demoted to
  an LZ4-compressed copy of their source HTML in RAM; and once too many
  of those pile up, the coldest spill to a small on-disk swap file
  instead -- the browser's own swap, layered under the RAM one, freeing
  RAM further without needing the network. Waking a tab from either
  hibernation tier reconstructs it locally, no network round-trip, unlike
  a browser that discards and re-fetches. Covered by tests including one
  that opens 300 tabs and checks tabs land in the right tier and that the
  combined compressed footprint of the 292 non-active ones is far smaller
  than storing them uncompressed. See ARCHITECTURE.md for what this is
  and isn't (compresses source HTML and re-parses, not a live execution
  snapshot).
- **Tabs survive a restart.** The open tabs (url, title, history, and a
  compressed snapshot of each, read back from whichever tier it was in)
  are saved to a real per-user data directory as browsing happens, not
  just on a clean exit. On the next launch, a prompt offers to restore
  them ("N tab(s) from your last session are still residing. Restore
  them?") or start fresh. Verified live: opened 45 tabs (enough to
  exercise all three tiers), quit, relaunched, saw the real prompt with
  the right count, restored, and got all 45 back with the same one
  active -- which also caught and fixed a real `RefCell` double-borrow
  crash on waking a restored background tab; see ARCHITECTURE.md.
- **Hibernate.** A nav-bar button that closes the window after an
  explicit final save -- named that instead of "Quit" because nothing is
  discarded: every tab, whichever tier it's in, gets read back and
  written to the session file, so the next launch restores it from the
  local snapshot with no network refetch. Closing the window any other
  way (the OS close button, Alt+F4) gets the same guarantee, via a
  `on_close_requested` hook -- Hibernate is the discoverable, intentional
  version of something that's actually always true.
- **Customizable look and feel.** A settings panel (gear icon in the nav
  bar) offers Dark/Light themes for the browser chrome -- every themed
  color lives in one Slint `global`, switching applies live with no
  restart, and the choice persists across restarts. Only the chrome is
  themed; pages render as authored, same as any real browser's dark
  mode. Live-testing this caught and fixed a real layout bug (both the
  settings panel and the restore prompt were silently stretching to the
  full window height instead of sizing to their content); see
  ARCHITECTURE.md.
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
- **JavaScript actually runs**, via an embedded pure-Rust JS engine
  ([`boa_engine`](https://github.com/boa-dev/boa) — no V8, no system
  dependency). Inline `<script>` tags execute in document order against a
  shared per-page context, with a deliberately small set of real host
  bindings: `console.log/warn/error` and a `document.title` setter — not
  a full DOM (no `getElementById`, no events, no timers). A runaway script
  (`while (true) {}`) is bounded rather than hanging the browser. Verified
  live: a page whose script sets `document.title = "Changed by JS"` shows
  that title in the real tab strip, not just in a unit test. See
  ARCHITECTURE.md for the exact scope and why it's drawn there.
- **Builds for, and runs on, Android** (`aarch64-linux-android`) with the
  same UI code as desktop -- verified end to end on a real running
  emulator: a signed `.apk` installs, launches, updates in place across
  releases (a stable committed signing key, not a randomly-regenerated
  one -- see ARCHITECTURE.md), and renders real pages fetched live over
  the network, including a real, non-trivial site (duckduckgo.com's
  actual homepage, not a fallback page). A real chrome, purpose-fit for
  a phone screen: no more status-bar overlap, pill-shaped tabs/buttons
  sized for touch, an address bar that behaves like Fennec's (tapping it
  doesn't select the whole URL), and a compact overflow menu instead of
  a toolbar row that ran wider than the screen. A long list of real bugs
  were found and fixed getting here, from native-tls needing OpenSSL
  vendored in, to a launch-time crash from Android having no writable
  temp dir, to Melora's own address-bar handling silently dropping the
  URL scheme before it ever reached the page's own base URL -- see
  ARCHITECTURE.md's Android section for the full, evidence-backed list.
  Still open: the APK is debug-signed for sideloading, not release-signed
  for the Play Store.

All of the above was checked against a real running instance under Xvfb
(driven with `xdotool`, screenshotted with `xwd`), not just unit-tested —
see ARCHITECTURE.md for the specific before/after evidence on each one.

What's not wired up yet — see the roadmap in ARCHITECTURE.md:

- JavaScript is real but narrow: `console`/`document.title` only, no DOM
  API, so anything that needs real client-side rendering (beyond setting
  the page title or logging) won't appear.
- The underlying `blitz-dom` table-layout bug above is contained, not
  fixed — affected pages stop laying out fully partway through rather
  than crashing, which is progress but not a real fix.

Platform target: Linux and Android today (both with no system rendering
dependency), Windows and macOS previously verified building and not
expected to have broken, just not currently in the release matrix -- see
CI section below. iOS is deliberately out of scope, not dropped quietly
-- Apple requires all iOS browsers to render through WebKit, which would
mean compromising on "self-contained" specifically there; see
ARCHITECTURE.md.

## Building

```sh
cargo build
cargo test
cargo run
```

Requires a Rust toolchain with the 2024 edition (`rustc` ≥ 1.85) and, on
Linux, the usual windowing libraries (X11/Wayland + `libxkbcommon`).

To build the Android APK yourself: `rustup target add
aarch64-linux-android`, install the Android NDK and SDK (build-tools
35.0.0 specifically -- see ARCHITECTURE.md's Android section for why),
`cargo install cargo-apk`, then `cargo apk build --target
aarch64-linux-android --lib` from the repo root. The resulting APK lands
at `target/debug/apk/melora.apk`, debug-signed and ready to `adb install`
or copy to a device to sideload.

## CI and downloadable builds

[`.github/workflows/ci.yml`](./.github/workflows/ci.yml) runs `cargo
build`/`cargo test` on Linux on every push and pull request.
[`.github/workflows/release.yml`](./.github/workflows/release.yml) builds
a Linux x86_64 binary and an Android arm64 APK and attaches both to a
GitHub Release whenever a `v*` tag is pushed -- that's how to get an
actual build to try without building from source yourself. (Windows and
macOS were in this matrix earlier and built successfully then; they're
not in it right now since the immediate need was specifically a desktop
build and a phone-testable Android build -- re-adding them is a small,
independent change whenever they're wanted again.)

## License

GPL-3.0-only (see [LICENSE](./LICENSE)). Slint is used under its GPLv3
option, which requires the combined work to be GPLv3 — see
[Slint's licensing FAQ](https://github.com/slint-ui/slint/blob/master/FAQ.md)
for details.
