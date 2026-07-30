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
"300 tabs · 8 active · 292 compressed (14.2 KB) · 0 swapped (0 B)" — a
computed number, not a claim.

**On "more RAM-efficient than other browsers":** the compression tier is
a real, measured improvement over Melora's own previous discard-based
design, and a defensible one relative to how mainstream tab-discarding
works in general. What isn't done, and shouldn't be claimed, is a
head-to-head memory benchmark against Chrome/Firefox/Safari — that needs
real profiling on real hardware outside this environment, not a number
invented here.

## The disk swap tier (a third tier underneath RAM compression)

The RAM-compressed tier above still holds one LZ4 blob per background tab
in memory. For a session with hundreds of tabs, even that adds up, so
there's a second hibernation tier underneath it: `swap.rs`'s `SwapFile`,
a small on-disk overflow file. Once more than `ram_compressed_budget`
(32, `DEFAULT_RAM_COMPRESSED_BUDGET` in `src/tabs.rs`) tabs are RAM-
compressed, `TabManager::enforce_budget` writes the coldest ones' bytes
to this file instead and moves them to `TabState::Swapped`
(`Tab::swap_slot` records where). Waking a swapped tab
(`TabManager::activate`) reads those bytes back, decompresses, and
re-parses -- the same "no network round-trip" property the RAM tier has,
just with a little disk I/O the RAM case doesn't pay. This is the
literal "small swapfile" the browser was asked to have: OS-level zram
compresses cold *process* memory; this compresses cold *tab* memory one
level further, to disk, using the same LZ4 representation as the RAM
tier so the two are just two ends of one spectrum, not two designs.

Deliberately simple, and documented as such rather than hidden: the swap
file is append-only for the lifetime of one run (space from a tab that
wakes or closes is never reclaimed -- a real allocator would track a
free list) and is deleted on process exit (`SwapFile`'s `Drop`), because
it's a RAM extension for the *current* run, not persistent storage --
see Session persistence below for the separate mechanism that actually
survives a restart. If the disk write itself fails (e.g. no space left),
`enforce_budget` leaves the tab's bytes in RAM rather than losing them.

Verified in `src/tabs.rs`'s tests:
- `compressed_tabs_beyond_the_ram_budget_spill_to_the_disk_swap_tier` —
  demotes enough tabs to exceed a (test-only, lowered) RAM budget and
  checks the overflow actually lands in `TabState::Swapped` with an
  empty RAM footprint.
- `waking_a_swapped_tab_reads_it_back_from_disk` — wakes a swapped tab
  with no HTML supplied and checks it reconstructs to the same node
  count, the same "no network" proof the RAM-tier test does.

Verified live, not just in tests: opening enough tabs in the real running
app (under Xvfb) to exceed the RAM budget produced real entries in the
"swapped" stat in the status bar, and clicking one of those tabs back
into view worked -- woke it, decremented the swapped count, incremented
active, and rendered its real content. This same session also caught a
real bug, described under Session persistence below.

## Session persistence (surviving a restart)

`session.rs`'s `SessionStore` is what makes "tabs survive a restart" a
reality rather than a request for a permanent RAM extension. It's a
distinct file from `SwapFile` on purpose: swap is ephemeral and deleted
on exit; the session file lives in a real per-user data directory
(`$XDG_DATA_HOME`/`~/.local/share`, `~/Library/Application Support`, or
`%APPDATA%` depending on platform -- resolved by hand in `data_dir()`
rather than pulling in a `directories` crate, consistent with this
project's "no dependency it doesn't need" approach) and is meant to
outlive the process.

**What's saved:** `TabManager::session_snapshot` builds one record per
open tab -- url, title, history, history position, and LZ4-compressed
HTML -- regardless of which tier the tab is currently in (`Active`
tabs get their live HTML compressed fresh; `Compressed` tabs' bytes are
already in the right form; `Swapped` tabs get read back from disk).
`main.rs` calls this and writes the result after essentially every
tab-mutating action (`persist_session`, called after every `refresh` in
`main.rs`), not just on a clean exit -- so the saved session reflects
reality even if the process is later killed rather than closed properly.
Saves are best-effort (`let _ = session_store.save(...)`): a failure to
persist shouldn't interrupt browsing, only lose the ability to restore.

**What happens at startup:** before opening any tab, `main` calls
`SessionStore::load`. If it's empty (first run, or the last run already
restored/discarded), Melora opens `melora://start` as before. If it's
not, nothing is opened yet -- instead the chrome shows a modal prompt
("N tab(s) from your last session are still residing. Restore them?",
the "popup mentioning residing tabs" this feature was asked for) with
**Restore** and **Start Fresh** buttons (`show-restore-prompt` /
`restore-prompt-text` properties and `restore-session` / `discard-session`
callbacks in `melora.slint`, a conditional overlay `Rectangle` declared
after the main layout so it paints on top). Restoring calls
`TabManager::restore_session`, which reconstructs every tab directly in
the `Compressed` tier (skipping a live parse for tabs that may not be
looked at for a while -- restoring hundreds of tabs shouldn't lay all of
them out up front) and returns whichever tab was marked active when the
session was saved, which the caller then `activate`s to bring live.
Declining clears the stale session file and starts fresh, same as a
first run.

Verified in `src/tabs.rs`'s tests
(`session_snapshot_and_restore_round_trips_url_title_history_and_content`,
`session_snapshot_includes_swapped_tabs_by_reading_them_back_from_disk`)
and in `src/session.rs`'s own tests (round-tripping the on-disk binary
format, a corrupt file being reported as an error rather than silently
treated as empty, `clear` being a no-op when there's nothing to clear).

**Verified live, and it caught a real bug.** Opening 45 tabs in the real
app, quitting, and relaunching produced the actual restore prompt ("45
tab(s) from your last session are still residing"), and clicking Restore
correctly reconstructed all 45 (status bar: "45 tabs · 1 active · 31
compressed · 13 swapped", matching the pre-restart tiering) with the
same tab active as before. Clicking one of the restored background tabs
to wake it, though, crashed the process the first time: `main.rs`'s
`on_activate_tab` handler had `match manager.borrow_mut().activate(id) {
... }`, and Rust's temporary-lifetime rules keep the `RefMut` from
`borrow_mut()` alive for the *entire* match, not just the scrutinee --
so the moment an arm called `manager.borrow()` (to refresh the UI), the
`RefCell` panicked with "already mutably borrowed". This is a well-known
Rust footgun, and the pattern predates this milestone -- it just hadn't
been live-exercised before, because waking a compressed/swapped tab via
a real click had never actually been part of an earlier live-verification
pass. Fixed by binding the match scrutinee to a local first
(`let result = manager.borrow_mut().activate(id); match result { ... }`),
which drops the borrow before the match body runs. Confirmed by
re-running the exact steps that crashed it -- restore 45 tabs, click a
background tab -- and seeing it wake correctly (swapped count
decrementing, the real page content appearing) with the process still
alive afterward.

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

## JavaScript

Inline `<script>` tags now actually run, via [`boa_engine`](https://github.com/boa-dev/boa)
(pure-Rust, no V8/system dependency -- picked specifically to keep the
"no system dependency of any kind" property everywhere else in this repo;
`gosub_v8`, the other JS path on the table, is a huge C++ build and was
ruled out as infeasible in this environment, let alone as a good fit for
a self-contained binary). `src/js.rs`'s `JsEngine` wraps one `boa_engine::Context`
per page and binds a deliberately small set of host functions:

- `console.log` / `console.warn` / `console.error` -- captured into a log
  (`JsEngine::take_console`), not printed anywhere yet.
- `document.title = "..."` -- a property *setter only* (`ObjectInitializer::accessor`
  with no getter), captured into `JsEngine::title`.

That's it. **This is not a DOM.** There's no `getElementById`, no element
tree exposed to script, no event listeners, no `fetch`/`XMLHttpRequest`, no
timers. A page that reads `document.title` back, queries the DOM, or relies
on any other browser API will see `undefined`/throw, same as it would in
an engine that never ran the script at all -- the difference this milestone
makes is narrow and specific: simple scripts that log or set the page
title (a surprisingly common real pattern -- SPA loading-state titles,
analytics beacons that just log) now work, and it establishes the
plumbing (a real embedded JS engine, wired into the page lifecycle) that
a future, larger DOM-binding effort would build on rather than starting
from zero.

**Where scripts run:** `PageEngine::from_html` walks the parsed document
(`collect_inline_scripts`, a stack-based pre-order DFS from the root,
skipping any `<script src="...">` since external scripts aren't fetched)
and runs every inline script it finds, in document order, against **one
shared `JsEngine`** -- so `<script>var x = 1;</script>...<script>x++;</script>`
sees the same global scope, matching how real browsers run multiple
`<script>` blocks on one page. This happens exactly once, right after the
initial layout resolve; scripts are *not* re-run when a sub-resource
arrives later (`apply_resource`), since re-running on every stylesheet/
image load would mean duplicate console spam and the title getting reset
over and over for no reason.

**Safety:** script execution is synchronous on the same call path as
parsing, so a runaway script (`while (true) {}`) would otherwise hang the
whole browser, not just its own tab. `JsEngine::new` sets
`context.runtime_limits_mut().set_loop_iteration_limit(1_000_000)`, so a
loop that never terminates errors out instead of hanging -- verified in
`src/js.rs`'s `a_runaway_loop_is_bounded_instead_of_hanging` test. Thrown
exceptions and syntax errors are likewise caught and logged rather than
propagated (`a_thrown_exception_is_captured_not_propagated`) -- a broken
script shouldn't take the page down any more than a missing image does.
As defense in depth, `PageEngine::from_html` also wraps the whole
collect-and-run step in `catch_unwind`, the same pattern already used for
layout (see Crash containment) -- so even a panic inside `boa_engine`
itself, or in the tree-walk, degrades to "no script output" rather than
crashing the process.

**Where it surfaces today:** a script's `document.title` override, once
set, is threaded through to `Tab::title` (`TabManager::open_tab`, `load`,
and the wake-from-compressed path in `activate` all now prefer
`PageEngine::title_override()` over the raw URL when present) -- so the
tab strip shows a script-set title, not just the address that was typed.
`console.*` output is captured (`PageEngine::console_log`) but not shown
in the UI yet; there's no devtools-style panel to put it in, so it's
plumbed through and unused for now rather than left off entirely.

The GC-safety mechanics (Boa's `Trace`/`Finalize` traits require captured
closure state to implement `Trace`; a plain `Rc<RefCell<...>>` doesn't) are
handled with a small newtype (`HostState` in `src/js.rs`) that opts out of
GC tracing via `unsafe impl Trace { empty_trace!(); }` -- sound here
specifically because the wrapped state is plain Rust strings, not JS/GC
values, so there's nothing for the collector to need to trace into.

## Does Melora load ads?

Asked directly, worth answering directly: **mostly no, but not because
anything blocks them -- because the engine is currently too limited to
run what actually delivers them.** Worth being precise about why, since
"no ads" for the wrong reason is a fragile property, not a feature:

- **Static `<img>`-tag ads would load.** `blitz-dom` fetches every
  sub-resource a page's HTML actually references (see Sub-resource
  loading above) with no filtering of any kind -- an ad image linked
  directly in the server-rendered markup loads exactly like any other
  image.
- **JS-injected ads mostly can't, because two different limitations both
  have to be true at once for that to matter, and both currently are.**
  Modern ad delivery is almost entirely JavaScript: a network's script
  tag runs, then creates the actual ad markup (an `<iframe>`, an `<img>`,
  more `<script>` tags) at runtime. Melora currently blocks this whole
  path twice over: *external* `<script src="...">` tags aren't fetched
  or run at all (see JavaScript above -- only inline scripts execute),
  and even an inline script that ran the same logic couldn't act on it,
  since the JS bindings don't expose any DOM-mutation API (`document`
  only has a `title` setter -- no `createElement`, no `appendChild`, no
  `innerHTML`). So the dominant real-world ad-delivery mechanism is
  unreachable from two independent directions right now.
- **`<iframe>` isn't implemented at all** (not by Blitz, not by anything
  in `src/engine.rs`), which rules out the other extremely common ad
  format (an iframe pointing at an ad network's own page) regardless of
  script execution.

None of this was designed as ad-blocking -- it's what "JavaScript
support" narrowly scoped to `console`/`document.title` and no `<iframe>`
support happens to imply as a side effect. It'll stop being true the
moment the JS binding surface grows a real DOM API (roadmap below), at
which point ads will load exactly as well as any other JS-driven content
does, same as a real browser, unless something is deliberately built to
prevent it -- see the network-layer blocklist and WASM extension items
below, which is where actual, intentional ad-blocking would have to live.

## What's still stubbed, and why

- **JavaScript is real but narrow.** See the JavaScript section above --
  `console`/`document.title` only, no DOM API, no events, no timers, no
  network from script.
- **The disk swap tier never reclaims space within a run.** See the disk
  swap tier section above -- append-only by design, deleted whole on
  exit; a real free-list is future work if it ever matters in practice.
- **No extension system of any kind.** No content-blocking, no
  WebExtensions compatibility, nothing. See Roadmap.

## Roadmap (rough order)

1. Report or work around the `blitz-dom` table-layout panic upstream, so
   affected pages (like pypi.org) render fully instead of stopping partway
   through layout once it's hit (contained now, not yet fixed).
2. Revisit memory/CPU budgets with real profiling data instead of the
   current fixed `max_active = 8` / `ram_compressed_budget = 32` constants.
3. Decide gosub-engine's role: it has its own layout/render pipeline
   (`gosub_render_pipeline`, `gosub_renderer_vello`) and even a JS engine
   binding (`gosub_v8`) that Blitz doesn't — evaluate whether it should
   replace Blitz as the primary engine, run as a selectable second engine,
   or stay as reference-only. Not decided yet; `tests/gosub_html5_smoke.rs`
   is the groundwork for making that call with real data instead of a
   guess.
4. Grow the JS binding surface beyond `console`/`document.title` -- a real
   `getElementById`/DOM-mutation API, once there's a design for keeping
   `blitz-dom`'s tree and Boa's JS values in sync safely -- rather than
   the deliberately narrow bindings in place now.
5. Give the disk swap tier a real free list, if profiling ever shows the
   append-only growth within a single long-running session actually
   matters.
6. A network-layer domain/URL blocklist (à la uBlock Origin's network
   filtering, minus the extension machinery) -- checked in `src/net.rs`
   before a request is ever issued. This is real, intentional ad/tracker
   blocking, and it's a much smaller project than an extension system:
   no JS DOM API, no sandboxing model, no extension format needed, just
   a list and a lookup. Worth doing before, or independent of, item 7.
7. A WASM-based extension system, so something like uBlock Origin's
   *actual logic* (not the WebExtensions-format package itself, which
   assumes a Chromium/Firefox host) could run inside Melora -- a
   sandboxed WASM runtime (`wasmtime`/`wasmi`, both Rust, keeping the
   no-system-dependency property) with a deliberately-designed extension
   API surface (request interception at minimum for blocking; a content-
   script-style DOM API would need item 4 first). Substantial, open-
   ended work -- content-blocking specifically is much better served by
   item 6 first; this is for the day extensions in general (not just
   blocking) are wanted.
