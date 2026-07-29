# Architecture

## The premise, and why it's implemented the way it is

The original idea for Melora was to combine multiple dead, abandoned, or
incomplete Rust web browsers into one daily-driver browser. Taken literally
— splicing together the source trees of several independent, unfinished
browser projects — that isn't really something that can produce working
software: abandoned browser projects are abandoned or incomplete precisely
because browser engines are enormous (Servo and Chromium are millions of
lines built by large teams over years), and unfinished, architecturally
incompatible codebases don't merge into something more finished than any of
them.

What *does* work, and is what Melora actually does: several of these
projects are dead as standalone browsers but live on as real, maintained
Rust crates, because their authors split the reusable parts out. Melora
combines those surviving parts, rather than the dead shells around them.

## Survey of the landscape (Rust browser/engine projects)

| Project | State | Relevance |
|---|---|---|
| [Servo](https://github.com/servo/servo) | Abandoned by Mozilla 2020–2023, now revived under Linux Foundation Europe (0.1.0 released April 2026) | Source of `html5ever`, `cssparser`/`selectors`, Stylo (style system), WebRender. Most Rust browser work descends from this. |
| [Verso](https://github.com/versotile-org/verso) | A full browser built on Servo; **archived/unmaintained** — couldn't keep pace with upstream Servo | The closest thing to "an abandoned Rust browser" in the literal sense. Its lesson (a full custom shell around Servo is expensive to keep in sync) is why Melora doesn't attempt the same thing. |
| [Kosmonaut](https://github.com/twilco/kosmonaut) | Abandoned ~2021 | Toy/experimental engine using `html5ever`/`cssparser`; confirms those crates are the reusable part, not the abandoned shells built on top of them. |
| [Robinson](https://github.com/mbrubeck/robinson) | Abandoned tutorial project (2014-era) | Educational "let's build a browser engine" toy; too minimal to build on, but the direct ancestor of the "compose an engine from small Rust pieces" approach Melora and Kosmonaut both take. |
| [Blitz](https://github.com/DioxusLabs/blitz) | **Actively maintained** (Dioxus Labs) | Not itself abandoned, but it *is* the literal reassembly this project's premise describes: a modular HTML/CSS engine built from `html5ever`, `selectors`, Stylo, and Taffy — the same lineage as the abandoned projects above. This is what Melora currently uses for parsing/style/layout. |

Conclusion: the honest way to "combine dead/abandoned/incomplete Rust
browsers" is to build on the crates that survived their host projects'
abandonment, via Blitz, rather than trying to literally merge dead
codebases (Verso/Kosmonaut) that don't share an architecture and mostly
don't build against current dependency versions.

## Chosen stack

- **Chrome UI:** [Slint](https://slint.dev), under its GPLv3 license option
  (hence this repo is GPL-3.0-only — see FAQ link in README).
- **Page engine:** `blitz-dom` + `blitz-html` + `blitz-traits`
  (`HtmlDocument::from_html` → `.resolve()` runs HTML parsing, CSS style
  resolution, and Taffy layout). Verified working in this repo — see
  `src/engine.rs` and its test.
- **Tab model:** custom (`src/tabs.rs`) — this is Melora's own contribution,
  not sourced from any of the above.

## Tab hibernation (the "hundreds of tabs, lightweight" feature)

`TabManager` keeps an LRU order over open tabs and a fixed budget
(`max_active`, currently 8) of tabs allowed to hold a live `PageEngine` —
the actual parsed DOM/style/layout tree, which is the expensive part of a
browser tab. Every tab beyond the budget is hibernated: its `PageEngine` is
dropped and only `url`, `title`, and browsing history are kept (a handful of
short strings). Reactivating a hibernated tab reconstructs its engine.

This is the same idea real browsers (Chrome's tab discarding, Safari's tab
suspension) use for exactly this problem, implemented here as a small,
independently testable component — see the `hundreds_of_tabs_stay_within_the_active_budget`
test in `src/tabs.rs`, which opens 300 tabs and asserts only the budgeted
few stay resident.

## What's stubbed, and why

- **Networking.** Tabs currently render a generated placeholder page
  describing what would happen, rather than fetching the real URL. Wiring
  up `blitz-net` (which exists and would provide this) needs an async
  runtime and a `NetProvider` implementation plumbed through `TabManager`;
  that's a real chunk of work and was left as a clearly-marked next step
  rather than faked.
- **Painting.** The content pane currently shows layout stats as text.
  Rendering the actual laid-out page requires `blitz-paint` (or
  `anyrender`) rasterizing into a pixel buffer that gets blitted into a
  Slint `Image` element each frame. This is the next concrete milestone —
  the parse/style/layout side is already proven to work (see
  `src/engine.rs`); only the raster-to-Slint-surface step remains.

## Roadmap (rough order)

1. Wire `blitz-net` for real HTTP(S) fetching, replacing the placeholder
   HTML generator.
2. Rasterize the laid-out document (via `blitz-paint`) into a buffer and
   display it in the Slint content pane, replacing the text stats view.
3. Forward input events (mouse, scroll, keyboard) from the Slint content
   area into `blitz-dom`'s hit-testing/event handling, so pages become
   interactive.
4. Persist hibernated-tab metadata to disk so a session with hundreds of
   tabs survives a restart without re-fetching everything at once.
5. Revisit memory/CPU budgets with real profiling data instead of the
   current fixed `max_active = 8` constant.
