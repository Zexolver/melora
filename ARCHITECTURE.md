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

The table below started as a general web search, then was superseded by the
project owner's own curated GitHub stars list —
[rusty-web-browsers-any](https://github.com/stars/Zexolver/lists/rusty-web-browsers-any)
— which is now the primary source. Every entry was checked directly
(license file, last activity, workspace layout), not taken on faith from
the list's descriptions.

| Project | State | License | Relevance |
|---|---|---|---|
| [Servo](https://github.com/servo/servo) | Abandoned by Mozilla 2020–2023, now revived under Linux Foundation Europe | MPL-2.0 | Source of `html5ever`, `cssparser`/`selectors`, Stylo, WebRender. Most Rust browser work descends from this. Not on the stars list, but everything below is downstream of it. |
| [Verso](https://github.com/versotile-org/verso) | Full browser on Servo; **archived/unmaintained** | MPL-2.0 | The closest thing to "an abandoned Rust browser" literally. Its lesson — a full shell around a fast-moving upstream engine is expensive to keep in sync — is why Melora doesn't attempt the same thing. |
| **[gosub-engine](https://github.com/gosub-io/gosub-engine)** | **Actively developed**, own registry at registry.gosub.io | MIT | From the stars list. Not abandoned, but it *is* the closest thing to Melora's own goal: a modular, embeddable engine with its own HTML5 parser (`gosub_html5`), CSS3 parser (`gosub_css3`), layout (`gosub_render_pipeline`), and even a JS engine binding (`gosub_v8`). **Actually integrated** — see below. |
| [OkuBrowser/oku](https://github.com/OkuBrowser/oku) | Active | AGPL-3.0 | From the stars list. A real browser, reportedly Servo/Verso-based. Not integrated: AGPL is one-directionally compatible with GPLv3 — combining it in would force this whole project to become AGPL, which wasn't a decision to make silently. Noted here for a future explicit choice. |
| [ZeroX-DG/moon](https://github.com/ZeroX-DG/moon) | Hobby project, low activity | MIT | From the stars list. From-scratch HTML/CSS/layout/GPU-render pipeline, not published as reusable crates (no crates.io/registry entries) — would need vendoring, not `cargo add`/git-dependency. |
| [MQuy/mbrowser](https://github.com/MQuy/mbrowser) | Toy, inactive | MIT | From the stars list. Servo-inspired toy DOM/CSSOM; educational scope, not a reusable crate. |
| [simonwuelker/Stormlicht](https://github.com/simonwuelker/Stormlicht) | WIP, low activity | MIT | From the stars list. Serious solo effort (has an ACID-test harness) but not published as reusable crates. |
| [Kosmonaut](https://github.com/twilco/kosmonaut) | Abandoned ~2021 | MPL-2.0 | Toy engine using `html5ever`/`cssparser`; confirms those crates are the reusable part, not the abandoned shell built on top of them. |
| [Robinson](https://github.com/mbrubeck/robinson) | Abandoned tutorial (2014-era) | — | Educational "let's build a browser engine" toy; too minimal to build on, but the direct ancestor of the "compose an engine from small pieces" approach every project on this list takes. |
| [Blitz](https://github.com/DioxusLabs/blitz) | **Actively maintained** (Dioxus Labs) | MIT/Apache-2.0 | Not on the stars list, not itself abandoned — but it's the literal reassembly the project's premise describes: `html5ever` + Stylo + Taffy recombined into an embeddable engine. **Currently Melora's default engine.** |
| naglfar, wasabi, kamaitachi, egui_browser, orionium, webx, feca, nami-browser, Shinyaic, abhaynayar/browser, violet, serval, ritsource/browser, harbor, d0iasm/vulbr | Toy/learning "browser from scratch" projects, mostly inactive | Various/none stated | From the stars list. Genuine examples of the "dead/incomplete Rust browser" premise, but each is a standalone binary (not a library), pre-1.0, and not built against current dependency versions — no `cargo add`/git-dependency integration path without a vendoring effort per project. |
| antoyo/titanium | Abandoned | — | From the stars list. A keyboard-driven *shell* around WebKitGTK (via `webkit2gtk`), not a Rust-native engine — out of scope for an "engine written in Rust" combination. |
| catacombing/kumo | Moved off GitHub, to SourceHut | — | From the stars list. Not evaluated further; no longer reachable at the starred URL. |
| webui-dev/rust-webui, chinedufn/percy, rail44/squark, newDINO/web-fs, rchaser53/rust-jvm, surrealdb/indxdb, OmniKee/OmniKee, NicholasDrian/web_cad | Active, unrelated | Various | On the stars list but not browser engines: a WebView-binding UI toolkit, two Rust/Wasm *frontend* frameworks (build web apps, not browsers), a Wasm filesystem shim, a JVM-in-Wasm toy, a WASM IndexedDB layer, a password manager, and a browser-targeted CAD engine. Likely stars of general interest rather than candidates — flagged here rather than silently dropped, in case the list should be trimmed. |

Conclusion: the honest way to "combine dead/abandoned/incomplete Rust
browsers" is to build on the pieces that survived their host projects —
either as crates already reassembled (Blitz) or as an actively-developed
project explicitly built to be that reassembly (gosub-engine) — rather than
vendoring genuinely dead, pre-1.0, non-library toy codebases wholesale.

## Chosen stack

- **Chrome UI:** [Slint](https://slint.dev), under its GPLv3 license option
  (hence this repo is GPL-3.0-only — see FAQ link in README).
- **Page engine (default, wired into tabs):** `blitz-dom` + `blitz-html` +
  `blitz-traits` (`HtmlDocument::from_html` → `.resolve()` runs HTML
  parsing, CSS style resolution, and Taffy layout). Verified working in
  this repo — see `src/engine.rs` and its test.
- **Page engine (evaluated, not yet wired in):** gosub-engine's
  `gosub_html5` + `gosub_css3`, pulled as git dependencies directly from
  [gosub-io/gosub-engine](https://github.com/gosub-io/gosub-engine) (MIT).
  Their crates aren't published to crates.io — they publish to their own
  registry (`registry.gosub.io`), which is why `.cargo/config.toml`
  declares that registry: Cargo needs it to parse gosub's manifests even
  though we resolve the crates via `git`, not the registry itself. Proven
  working — see `tests/gosub_html5_smoke.rs`, which parses a fragment with
  gosub's own from-scratch HTML5 parser and checks the resulting tree.
  This dependency is `[dev-dependencies]`-only for now, so it doesn't
  affect `cargo build`/`cargo run`, only `cargo test`. Whether to replace
  Blitz with gosub-engine outright, run both behind a feature flag, or
  drop this evaluation is an open decision — see Roadmap.
- **Tab model:** custom (`src/tabs.rs`) — this is Melora's own contribution,
  not sourced from any of the above.

## Platform scope

Melora targets desktop (Linux, Windows, macOS) with **no system
dependency for rendering** — not a platform WebView, not WebKit, nothing.
Every tab's engine is Melora's own code (Blitz today, gosub-engine under
evaluation), the same on every desktop platform; the only thing that
varies per OS is windowing, handled by Slint's existing cross-platform
backend.

Mobile is explicitly out of scope for now, not quietly dropped: iOS
specifically cannot use a self-contained custom engine at all — Apple
requires every iOS App Store browser to render through WebKit
(`WKWebView`), full stop, which would mean either an actual system
dependency there or not shipping on iOS. Since the point of this project
is a genuinely self-contained engine, iOS is left out of the platform
list rather than compromised on. Android doesn't have that restriction and
could plausibly run the same Rust engine + Slint UI later, but that's real,
separate cross-compilation work that hasn't been attempted yet.

**UI toolkit:** Slint, chosen for its GPLv3 option (see License in
README) and because it has, in practice, worked without issue through
every milestone so far — it compiles, runs headlessly under Xvfb, and its
callback/property model has cleanly supported everything built on top of
it, including bridging a background network thread back to the UI thread
safely (see Networking, below). [iced](https://iced.rs) (also
GPL-compatible) is a reasonable fallback if Slint hits a real blocker —
most plausibly during future mobile/Android work, where Slint's platform
support would need to be re-evaluated — but switching now would mean
discarding working, tested UI code to solve a problem that hasn't
occurred. Noted here as the contingency, not something being built.

## Tab compression (the "hundreds of tabs, low RAM" feature)

`TabManager` keeps an LRU order over open tabs and a fixed budget
(`max_active`, currently 8) of tabs allowed to hold a live `PageEngine` —
the actual parsed DOM/style/layout tree, which is the expensive part of a
browser tab. This part is the same idea real browsers use (Chrome's tab
discarding, Safari's tab suspension): most open tabs don't need to be fully
resident at once.

Where Melora deliberately differs from "discard and refetch": every tab
beyond the budget is demoted to an **LZ4-compressed copy of its source HTML
kept in RAM** (`Tab::compressed_html`, `TabManager::enforce_budget` in
`src/tabs.rs`), not dropped outright. Reactivating a tab decompresses and
re-parses that snapshot locally — `TabManager::activate` — with **no
network round-trip and no dependency on being online**, unlike a browser
that literally discards a tab's state and has to re-fetch it from the
internet when you switch back. This is the "zram for tabs" the design is
named after: trade a little CPU (fast LZ4 compression) and a little RAM
(the compressed snapshot, typically a small fraction of the original HTML,
and tiny next to a live DOM/style/layout tree) for not needing the network
at all to restore a tab.

What this doesn't do: it isn't a byte-for-byte process-memory snapshot the
way OS-level zram is. It compresses the *source* HTML and re-parses on
wake, not a live execution/scroll/form-input state — because Melora
doesn't have in-page interactivity wired up yet (see Roadmap). Once it
does, extending the snapshot to include that state is the natural next
step, not a redesign.

Verified in `src/tabs.rs`'s tests:
- `demoting_a_tab_actually_compresses_its_content` — asserts the
  compressed snapshot is real (non-empty, smaller than the source), not
  just a size-zero stub.
- `waking_a_compressed_tab_needs_no_externally_supplied_html` — wakes a
  tab and checks it's restored to the same DOM node count, calling
  `activate` with no HTML argument at all, proving the network genuinely
  isn't involved.
- `hundreds_of_tabs_stay_within_the_active_budget_and_compressed_total_stays_small`
  — opens 300 tabs and asserts both that only 8 stay active *and* that the
  total compressed footprint across the other 292 is far smaller than
  storing them uncompressed would be.

The status bar surfaces this directly (`format_bytes` in `main.rs`): e.g.
"300 tabs · 8 active · 292 compressed (14.2 KB)" — a computed number, not a
claim.

**On "more RAM-efficient than other browsers":** the compression tier is
a real, measured improvement over Melora's own previous discard-based
design, and a defensible one relative to how mainstream tab-discarding
works in general. What isn't done, and shouldn't be claimed, is a
head-to-head memory benchmark against Chrome/Firefox/Safari — that needs
real profiling on real hardware outside this environment, not a number
invented here.

## Networking

`src/net.rs` wires up real HTTP(S) fetching via `blitz-net`, replacing the
generated placeholder pages from the first scaffold. The interesting part
isn't the fetch itself, it's the thread boundary: `blitz-net` needs a tokio
runtime, and Slint's UI event loop is single-threaded and holds types that
aren't `Send`. `Network::spawn` owns a dedicated OS thread running a tokio
runtime that does the actual fetching; only `Send`-safe data (a `TabId`,
the target `Url`, the resulting bytes, and a `NavIntent` describing which
`TabManager` method to call once the fetch lands) ever crosses the thread
boundary. `main.rs` drains completed fetches on the UI thread with a
`slint::Timer`, where it's safe to touch `TabManager` and Slint's
generated types. This is proven end-to-end by a test in `src/net.rs`
(`fetch_result_flows_through_to_a_navigated_tab`) that runs the real
background thread and fetch pipeline against a `file://` URL — hermetic,
so it doesn't depend on network access being available wherever it runs —
and by manual testing against real HTTP(S) URLs.

`melora://start` and `melora://new-tab` stay locally generated (no fetch);
everything else goes through the network layer, including reload,
back/forward, and (only as a fallback -- see Tab compression above) waking
a tab whose local snapshot is somehow missing.

## Painting

`PageEngine::paint` (`src/engine.rs`) rasterizes the current, already-laid-
out document to a straight-alpha RGBA8 buffer via `blitz-paint` +
`anyrender_vello_cpu` -- a CPU-only Vello backend, chosen specifically so
this doesn't need a GPU surface or window handle: it just produces plain
bytes, which keeps `PageEngine` decoupled from whatever UI toolkit ends up
displaying them (relevant given Slint has a documented iced fallback --
see Platform scope). `main.rs` wraps that buffer as a
`slint::SharedPixelBuffer<Rgba8Pixel>` / `slint::Image` and sets it on the
content pane's `Image` element, replacing the layout-stats text the
previous milestone showed. The pixel format needed no conversion --
verified empirically (render a single known-color div, inspect the byte
at its center) before writing a line of integration code, rather than
assumed from documentation.

Scrolling works the same way: a `TouchArea`'s `scroll-event` in
`melora.slint` forwards wheel deltas to `PageEngine::scroll_by`
(`blitz_dom::BaseDocument::scroll_viewport_by`), and the page is
repainted. The sign convention took an empirical pass to get right too --
`scroll_viewport_by`'s `dy` moves the *content*, not the viewport, so a
downward scroll (which should reveal content *below*) needs a *negative*
`dy`; this is documented on `PageEngine::scroll_by` and covered by
`scrolling_down_changes_what_is_painted` in `src/engine.rs`.

**A real bug this surfaced:** the first version of this milestone crashed
immediately on real pages -- `PageEngine::from_html` never set
`DocumentConfig::base_url`, so the moment a page's parser hit a relative
`<link href="/static/...">` (i.e. almost any real page), `blitz-dom`
panicked trying to resolve it against a missing base URL. This wasn't
caught by any unit test, because every test up to that point used
self-contained HTML with no relative references. It was caught by actually
running the built app under Xvfb, driving it with `xdotool` (typing a real
URL, clicking Go), and screenshotting the result via `xwd` -- which is
also how the fix was confirmed: same live app, same URL
(`https://pypi.org/`), before showing the panic's stack trace and after
showing the real, live-fetched PyPI homepage, unstyled (no CSS fetched
yet, see below) but genuinely rendered, followed by a working scroll down
to its real footer. `PageEngine::from_html` now takes the page's URL and
threads it into `config.base_url`; `resolves_relative_hrefs_against_the_page_url_instead_of_panicking`
in `src/engine.rs` is the regression test.

## Sub-resource loading (stylesheets, images, fonts)

Pages now fetch their own stylesheets, images, and fonts, not just the
top-level HTML -- this is what turns a real page from readable-but-plain
text into something that actually looks like the site. `blitz-dom`
dispatches these fetches itself as it parses (whenever it hits a
`<link rel=stylesheet>`, `<img src>`, `@font-face`, etc.) through whatever
`NetProvider<Resource>` is set on the document; `Network::spawn`
(`src/net.rs`) constructs exactly one such provider (a
`blitz_net::Provider<Resource>`, the same generic networking backend used
for top-level page fetches) and shares it across every tab via
`TabManager::set_resource_provider`. Results come back on the same
background thread as page fetches, through a second channel
(`NetworkEvents::resources`), and are applied the same way: drained on the
UI thread by `main.rs`'s existing timer, routed to the right tab by
matching `Resource`'s tagged doc id against each tab's `PageEngine::doc_id`
(`TabManager::apply_resource`), then `PageEngine::apply_resource` calls
`BaseDocument::load_resource` and re-resolves style + layout.

Proven two ways: a hermetic test (`resource_provider_delivers_a_fetched_stylesheet_to_the_right_doc`
in `src/net.rs`, plus an equivalent in `src/engine.rs`) that fetches a
`file://` stylesheet and checks the resulting pixels actually changed, and
live -- see Crash containment below, which walks through the real
before/after screenshots against pypi.org: unstyled black-text-on-white
before this milestone, real blue banners, the actual PyPI logo image, and
correct typography after it.

## Click-to-navigate

`PageEngine::hit_test_href` (`src/engine.rs`) hit-tests a point via
`BaseDocument::hit`, then walks up the hit node's ancestors
(`Node::parent`) looking for the nearest enclosing `<a href>` (the actual
hit target is usually the text or an inline element *inside* the link, not
the `<a>` itself), and resolves that `href` against the page's base URL.
`melora.slint`'s content-area `TouchArea` reports `clicked` with its
`mouse-x`/`mouse-y`, scaled in `main.rs` from the `TouchArea`'s actual
on-screen size to `PageEngine`'s raster size (they can differ, e.g. before
a resize settles) before hit-testing.

Verified two ways: unit tests in `src/engine.rs`
(`hit_test_href_resolves_a_relative_link_against_the_page_url`,
and a "clicking empty space finds nothing" negative case), and live --
a controlled `file://` test page (a full-viewport link to a second page)
loaded in the real running app, clicked via `xdotool`, screenshotted
before (a red block reading "CLICK ME TO NAVIGATE") and after (green,
"NAVIGATION SUCCEEDED") to confirm the whole chain -- hit-test, href
resolution, and the resulting `NavIntent::Navigate` -- actually fires from
a real pointer click, not just at the Rust-API level.

## Resize-driven re-layout

The content pane's `Rectangle` in `melora.slint` has `changed width` /
`changed height` handlers that fire `viewport-resized`; `main.rs` calls
`TabManager::resize_viewport`, which updates every currently-active tab's
`PageEngine` (`BaseDocument::set_viewport` + re-resolve) and remembers the
new size for tabs created or woken afterward. This replaces the earlier
"stretch the existing bitmap" behavior (`image-fit: fill` still exists in
the `.slint`, but now mostly just interpolates the sub-pixel gap between a
resize event landing and the next repaint, rather than doing all the work)
with a real re-layout at the new size.

Whether Slint fires `changed` callbacks reliably for built-in geometry
properties (as opposed to explicitly-declared ones, the only case its own
test suite covers) wasn't documented, so it was checked directly: a
minimal window with `changed width/height` on a `Rectangle`, run under
Xvfb, resized via `xdotool windowsize`, confirmed the callback fired with
the post-resize dimensions before this was built into the real app.
Confirmed again live in the full app afterward: a link's text visibly
rewrapped from two lines to one after narrowing the window, which only
happens with a genuine re-layout, not a stretched bitmap.

## Crash containment: a real bug in the underlying engine

Wiring up sub-resource loading surfaced a real, reproducible crash in
`blitz-dom` 0.1.4 itself: once pypi.org's real stylesheet was fetched and
applied, re-resolving layout panicked --
`index out of bounds: the len is 2 but the index is 2` inside
`blitz_dom::layout::table::TableTreeWrapper`'s grid-child-style lookup
(`layout/table.rs:333`), i.e. a bug in how that engine version emulates
HTML `<table>` layout via CSS Grid, most likely triggered by an irregular
table (rows with different cell counts) somewhere on the real page. This
is a bug in the third-party engine, not something patchable from here
without vendoring and modifying `blitz-dom`'s source -- out of scope for
this project. Several minimal repros were tried (jagged rows, colspan
mismatches, an empty row, `display: grid` overrides) and none reproduced
it in isolation; only the real page's actual CSS does. That's consistent
with immature layout code in a genuinely young (0.1.x) engine, which is a
real, honest cost of the "assemble from young/incomplete Rust engine
projects" premise this whole repo is built on -- not swept under the rug.

What matters for a daily driver is that one page's rendering bug can't
take the whole browser down. `PageEngine` now wraps every call into
`blitz-dom`'s layout resolution (`from_html`, `apply_resource`, `resize`)
and into painting (`paint`) in `std::panic::catch_unwind`
(`resolve_layout_safely` in `src/engine.rs`), converting a would-be crash
into "this operation didn't fully succeed" (`false`/`None`) instead of an
unwind that reaches `main`. The page may end up showing stale or
incomplete layout when this happens, but the process keeps running and
the rest of the browser -- other tabs, the chrome, further navigation --
is unaffected.

Verified live, not just reasoned about: the exact same steps that crashed
the process before the fix (navigate to `https://pypi.org/` in the real
running app under Xvfb) were re-run after it. Screenshots confirm all of
it together: the process stayed alive (checked via `ps`), the page shows
real styling and a real rendered PyPI logo image (proving sub-resource
loading works), and the panic is visible in the log (Rust's default panic
hook still prints it -- useful for debugging -- `catch_unwind` only stops
it from unwinding further) without taking the app down.

## What's still stubbed, and why

- **Persisting compressed tabs across restarts.** A session's hundreds of
  compressed snapshots live in memory only; closing Melora loses them, so
  reopening means re-fetching everything. Roadmap item, not attempted yet.
- **JavaScript.** Pages are static once loaded -- no script execution, so
  anything client-side-rendered (PyPI's own trending-packages section,
  for instance) won't appear. A large, separate undertaking (Blitz has no
  JS engine; gosub-engine's `gosub_v8` is the more promising path here --
  see the gosub-engine roadmap item below) rather than a small follow-up.

## Roadmap (rough order)

1. Report or work around the `blitz-dom` table-layout panic upstream, so
   affected pages (like pypi.org) render fully instead of stopping partway
   through layout once it's hit (contained now, not yet fixed).
2. Persist compressed-tab snapshots to disk so a session with hundreds of
   tabs survives a restart without re-fetching everything at once.
3. Revisit memory/CPU budgets with real profiling data instead of the
   current fixed `max_active = 8` constant.
4. Decide gosub-engine's role: it has its own layout/render pipeline
   (`gosub_render_pipeline`, `gosub_renderer_vello`) and even a JS engine
   binding (`gosub_v8`) that Blitz doesn't — evaluate whether it should
   replace Blitz as the primary engine, run as a selectable second engine,
   or stay as reference-only. Not decided yet; `tests/gosub_html5_smoke.rs`
   is the groundwork for making that call with real data instead of a
   guess. Also the more likely path to JavaScript support, whenever that
   becomes a priority.
