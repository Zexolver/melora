# rusty-web-browsers-any — research notes

Full research pass over all 135 repositories in
[Zexolver's rusty-web-browsers-any stars list](https://github.com/stars/Zexolver/lists/rusty-web-browsers-any).
Row numbers match [RUSTY_BROWSERS_URLS.md](./RUSTY_BROWSERS_URLS.md) — use
that file to jump straight to a repo's URL, this one for what it actually
is.

## Methodology and caveats

- Every repo was checked directly (its own GitHub page), not inferred from
  the stars list's own descriptions.
- Work was split across 9 parallel research passes (15 repos each) to get
  through all 135 without guessing on any of them.
- GitHub's relative "Updated X ago" timestamp text frequently didn't survive
  the fetch tooling's HTML-to-markdown conversion. Where that happened,
  "Status/Activity" falls back to commit count and any explicit
  Archived/read-only badge (which *did* come through reliably) — commit
  count is a weak proxy, not a real recency signal, and is flagged as such.
- Two repos 404'd on every attempt: `qrunk/Gemini` and `Dp5439/hyprbrowser`.
  Marked unreachable rather than guessed at — they may be renamed, deleted,
  or never existed at that path.
- License is "unknown" wherever no LICENSE file or license badge was
  visible on the repo page — not an assertion that the repo is unlicensed.

## Category key

- **Engine/Browser** — an actual web browser or browser-rendering-engine
  project; matches the "dead/abandoned/incomplete Rust browser" premise.
- **Tangential** — touches browsers somehow (automation, a WASM utility
  that runs in one, a browser extension's helper, testing tooling) but
  isn't itself a browser or engine.
- **Unrelated** — no meaningful connection to browsers beyond incidental
  vocabulary overlap.
- **Unreachable** — 404, couldn't be verified.
- **Ambiguous** — conflicting signals, would need a manual look.

## Highlights

- **[gosub-io/gosub-engine](https://github.com/gosub-io/gosub-engine)** —
  the one project on this list that's both a real, actively-developed,
  modular embeddable engine *and* MIT-licensed. Already integrated into
  Melora (see `tests/gosub_html5_smoke.rs` and `ARCHITECTURE.md`).
- **[abghim/airbrowse](https://github.com/abghim/airbrowse)** — a minimal
  macOS browser already built on **Slint + wry**, the same chrome-UI
  approach Melora uses. Worth a direct look for chrome-UI patterns even
  though it wraps WKWebView rather than a Rust-native engine.
- **[OkuBrowser/oku](https://github.com/OkuBrowser/oku)** — a real, active
  browser, but AGPL-3.0. Deliberately not integrated (see ARCHITECTURE.md)
  since combining AGPL code would force this whole GPLv3 project to become
  AGPL.
- **Closest literal matches to "abandoned Rust browser"** (archived/moved
  off GitHub, confirmed via an explicit badge, not just low activity):
  [SignalWalker/Construct](https://github.com/SignalWalker/Construct)
  (archived 2022), [DitherDude/browser](https://github.com/DitherDude/browser)
  (archived, moved to a self-hosted git),
  [catacombing/kumo](https://github.com/catacombing/kumo) (moved to
  SourceHut), [maekawatoshiki/naglfar](https://github.com/maekawatoshiki/naglfar)
  (archived by owner), [face-hh/webx](https://github.com/face-hh/webx)
  (archived, 1.9k stars — by far the most popular repo on this list).
- **Other serious hobby/solo engines** worth a closer look for
  vendoring/inspiration, none published as reusable crates:
  [simonwuelker/Stormlicht](https://github.com/simonwuelker/Stormlicht),
  [usadson/retina](https://github.com/usadson/retina) (modular monorepo,
  613 commits — the most substantial non-gosub engine found),
  [ZeroX-DG/moon](https://github.com/ZeroX-DG/moon),
  [MQuy/mbrowser](https://github.com/MQuy/mbrowser),
  [tathyagarg/harbor](https://github.com/tathyagarg/harbor) (432 commits).
- Roughly a third of the list turned out to be **not browsers at all**:
  WASM utility crates, browser-automation/AI-agent tools, password
  managers, chat apps, file servers, and demos that merely run *in* a
  browser via WASM. All flagged individually below rather than silently
  dropped.

## Full findings (all 135)

| # | Repo | Description | Language | License | Status/Activity | Stars | Category |
|---|---|---|---|---|---|---|---|
| 1 | gosub-io/gosub-engine | Embeddable, async browser engine written in Rust | Rust | MIT | Actively updated | 3.7k | Engine/Browser |
| 2 | maekawatoshiki/naglfar | Toy web browser implemented in Rust from scratch | Rust | MIT | Archived (Jul 2025) | 596 | Engine/Browser |
| 3 | hikalium/wasabi | Toy web browser + OS in Rust | Rust | MIT | Actively updated | 322 | Engine/Browser |
| 4 | antoyo/titanium | Keyboard-driven browser using WebKit2GTK | Rust | MIT | Low activity | 245 | Engine/Browser |
| 5 | ZeroX-DG/moon | Hobby web browser built from scratch | Rust | MIT | Low/moderate activity | 433 | Engine/Browser |
| 6 | d0iasm/vulbr | Toy single-process/thread browser (security course) | Rust | MIT | Low activity/educational | 33 | Engine/Browser |
| 7 | webui-dev/rust-webui | Rust bindings to use any browser/WebView as a GUI frontend | Rust | MIT | Actively updated | 74 | Tangential |
| 8 | keiya01/kamaitachi | Toy browser educational project | Rust | unknown | Low activity | 27 | Engine/Browser |
| 9 | OkuBrowser/oku | Browser with GTK/IPFS/P2P features | Rust | AGPL-3.0 | Actively updated | 207 | Engine/Browser |
| 10 | haojiang99/egui_browser | Test web browser using Rust and egui | Rust | MIT | Low activity | 13 | Engine/Browser |
| 11 | AnimeshAgarwal28/orionium | Web engine parsing HTML/DOM and rendering | Rust | GPL-3.0 | Low activity, early-stage | 2 | Engine/Browser |
| 12 | chinedufn/percy | Build frontend browser apps with Rust + WASM, SSR | Rust | Apache-2.0/MIT | Actively updated | 2.3k | Tangential |
| 13 | face-hh/webx | Alt-web ecosystem: custom browser, HTML/CSS engine, Lua, custom DNS | Rust | Apache-2.0 | Archived (Aug 2024) | 1.9k | Engine/Browser |
| 14 | rail44/squark | Rust frontend framework (virtual DOM) for browser and more | Rust | WTFPL | Low activity | 177 | Tangential |
| 15 | catacombing/kumo | Browser UI for portrait-mode touchscreen mobile | Rust | GPL-3.0 | Archived on GitHub, moved to SourceHut | 62 | Engine/Browser |
| 16 | surrealdb/indxdb | Key-value DB abstraction for IndexedDB in WASM | Rust | Apache-2.0 | Unclear | 110 | Unrelated |
| 17 | newDINO/web-fs | Wasm virtual file system for browsers | Rust | MIT/Apache-2.0 | Unclear | 9 | Unrelated |
| 18 | MQuy/mbrowser | Toy web browser from scratch | Rust | MIT | Unclear (143 commits) | 47 | Engine/Browser |
| 19 | rchaser53/rust-jvm | Toy JVM in Rust, runs in browser via WASM | Rust | unknown | Unclear (356 commits) | 63 | Tangential |
| 20 | simonwuelker/Stormlicht | Experimental browser engine from scratch | Rust | MIT | Ongoing hobby dev (1,570 commits) | 15 | Engine/Browser |
| 21 | dontpanic92/feca | Browser prototype (Felis layout/render + Catus JS interpreter) | Rust | MIT | Part-time project | 13 | Engine/Browser |
| 22 | lincolixavier/nami-browser | Simple browser from scratch | Rust | unknown | Low activity (7 commits) | 9 | Engine/Browser |
| 23 | Shinyaigeek/Shinyaic | Browser / HTML rendering engine | Rust | unknown | Unclear (219 commits) | 13 | Engine/Browser |
| 24 | abhaynayar/browser | Simple browser from first principles | unknown | MIT | Low activity (7 commits) | 4 | Engine/Browser |
| 25 | UMASHIBA1/violet | Web browser built for studying | Rust | unknown | Unclear (26 commits) | 14 | Engine/Browser |
| 26 | hayatoito/serval | Experimental web browser engine | Rust | unknown | Very early (2 commits) | 18 | Engine/Browser |
| 27 | ritsource/browser | Very simple browser: HTML/CSS parser, renderer, networking | Rust | unknown | Low activity (24 commits) | 6 | Engine/Browser |
| 28 | tathyagarg/harbor | Custom browser: HTTP client, HTML/CSS parsers, TTF, GPU rendering | Rust | MIT | Actively developed (432 commits) | 20 | Engine/Browser |
| 29 | OmniKee/OmniKee | Cross-platform KeePass-compatible password manager | Rust, TypeScript | AGPL-3.0 | Active | 23 | Unrelated |
| 30 | NicholasDrian/web_cad | CAD engine targeting WebGPU-capable browsers | Rust, WGSL, TS | MIT | Ongoing | 10 | Unrelated |
| 31 | w-henderson/WebPL | A Prolog interpreter for the browser | Rust | unknown | ~74 commits | 6 | Unrelated |
| 32 | erichdongubler-mozilla/moz-webgpu-cts | WPT metadata tooling for a WebGPU implementation | Rust | Apache-2.0/MIT | ~527 commits | 3 | Tangential |
| 33 | vertec-io/async_bevy_web | Render Bevy ECS state as HTML server-side | Rust | unknown | ~87 commits | 12 | Tangential |
| 34 | Hexilee/rover | A toy web browser | Rust | MIT | 3 commits | 4 | Engine/Browser |
| 35 | ustclug-dev/brotli-dec-wasm | Brotli decompressor for browsers/web workers via WASM | Rust | Apache-2.0/MIT | ~156 commits | 14 | Unrelated |
| 36 | shaunbennett/lucis-web | Raytracing in your web browser | JavaScript | MIT | WIP, ~17 commits | 4 | Unrelated |
| 37 | fwcd/trails | Experimental browser and rendering engine from scratch | Rust | GPL-3.0 | ~90 commits | 6 | Engine/Browser |
| 38 | mineichen/llm-web-chat | Showcase of Ollama usage within a browser | Rust | MIT | ~13 commits | 4 | Unrelated |
| 39 | privacy-ethereum/webtor-rs | Browser-focused Tor client, compiled to WASM | Rust | MIT | ~365 commits, v0.5.7 | 23 | Tangential |
| 40 | AustinHen/Web-Browser | HTML browser handling HTTP/1.1 and DOM rendering | Rust (inferred) | unknown | ~31 commits | 3 | Engine/Browser |
| 41 | mariluski/reSearch | "The web browser" / early-stage search engine (unclear which) | Rust | unknown | ~26 commits | 3 | Ambiguous |
| 42 | yazaldefilimone/webcore | An experimental web render engine | Rust | unknown | ~102 commits | 4 | Engine/Browser |
| 43 | hobo0xcc/browser | Web browser written in Rust | Rust | unknown | Unclear | 2 | Engine/Browser |
| 44 | lavignes/web | Hand-rolled no-script browser, aims for HTML spec compliance | Rust | unknown | ~11 commits | 1 | Engine/Browser |
| 45 | kntt32/ferrum | Browser and rendering engine (WHATWG HTML living standard) | Rust | MIT | ~76 commits, CI active | 5 | Engine/Browser |
| 46 | DrapNard/vulkan_browser_engine | High-perf engine: Rust + Vulkan, JS JIT, PWA, sandboxing | Rust | GPL-3.0 | Low activity (39 commits) | 5 | Engine/Browser |
| 47 | andrewjensen/skelly | Bare-bones browser, Chrome extension, targets reMarkable tablet | Rust | unknown | Low activity (138 commits) | 4 | Engine/Browser |
| 48 | sswahn/browser | Simple browser: nav controls, bookmarks, async HTTP | Rust | MIT | Low activity (164 commits) | 1 | Engine/Browser |
| 49 | AlexandrosLiaskos/Nous | Minimalist browser on wry/tao, Windows-only, ~600KB | Rust | MIT | Low activity (15 commits) | 3 | Engine/Browser |
| 50 | aryan-madan/Photon-Browser | Minimal browser: HTML render, CSS parse, JS via Boa | Rust | MIT | Actively updated, young (6 commits) | 3 | Engine/Browser |
| 51 | 3webs-org/vector | "A web browser. Currently prototype-quality." | Rust | unknown | Actively updated (213 commits, 11 open PRs) | 3 | Engine/Browser |
| 52 | voctal/epsilon | Full-featured web crawler and indexer with Postgres | Rust | Apache-2.0 | In development | 6 | Unrelated |
| 53 | rfwatson/weblog | Macros for console.log() etc. from Wasm | Rust | MIT/Apache-2.0 | Stable but inactive (11 commits) | 3 | Unrelated |
| 54 | DitherDude/browser | "web://" custom-protocol webstack, inspired by webx | Rust | MIT | Archived, moved to self-hosted git | 2 | Engine/Browser |
| 55 | abhishek-bhatkar/renderkit | Ground-up rendering engine to explore browser internals | Rust | MIT | Actively updated (24 commits) | 4 | Engine/Browser |
| 56 | usadson/retina | Standards-focused modular web engine monorepo | Rust | Apache-2.0 | Actively updated (613 commits, CI) | 14 | Engine/Browser |
| 57 | cakebaker/rbrowser | "Web Browser Engineering" book, Rust instead of Python | Rust | MIT | Low activity, ch.1 only (81 commits) | 3 | Engine/Browser |
| 58 | zTgx/llmweb-rs | Webpages to structured data via headless Chrome + LLMs | Rust | MIT | Actively maintained (43 commits) | 16 | Unrelated |
| 59 | npetrangelo/BFTML | Custom markup-language standard + browser implementation | Rust | MIT | Low/moderate (89 commits) | 4 | Tangential |
| 60 | remimimimimi/wbp | Lightweight from-scratch engine: HTML5/CSS2.2, DOM, winit render | Rust | AGPL-3.0 | Actively updated (15 commits, CI) | 1 | Engine/Browser |
| 61 | DevMaan707/surfai | AI-enhanced browser automation framework | Rust | MIT | Early-stage (~3 commits) | 9 | Tangential |
| 62 | toms74209200/tiny-browserbook | Browser implementation (educational, browser-book based) | Rust | MIT | Low activity (49 commits) | 1 | Engine/Browser |
| 63 | AllMight2099/whiteboard-browser | Canvas-style browser, inspired by browser.engineering | Rust | MIT | Low activity (12 commits) | 1 | Engine/Browser |
| 64 | puripuri2100/sabani | A toy web browser | Rust | unknown | Low activity (21 commits) | 1 | Engine/Browser |
| 65 | tzervas/webpuppet-rs | Self-hosted browser automation library, AI-provider integrations | Rust | MIT | Actively updated (48 commits) | 2 | Tangential |
| 66 | makalin/rustagent | AI web automation tool running in-browser via WASM | Rust | MIT | Low activity (5 commits) | 5 | Tangential |
| 67 | peikfeiring/rust-browser | Learning Rust with "Web Browser Engineering" book | Rust | MIT | Low activity (3 commits) | 1 | Engine/Browser |
| 68 | notTamion/web-runnables | Run local commands from your browser | unknown | MIT | Low activity (4 commits) | 1 | Unrelated |
| 69 | CEbbinghaus/file-browser | Small SSR web file browser (file manager UI) | Rust | unknown | Low activity (4 commits) | 1 | Unrelated |
| 70 | huzaifah0x00/rusty-web | Learning Rust by building a web browser | Rust | unknown | Low activity (7 commits) | 1 | Engine/Browser |
| 71 | Adamlip1334/finch-browser | A full-featured web engine written in Rust | Rust | unknown | Low activity (14 commits) | 1 | Engine/Browser |
| 72 | stanig2106/Fluxa | Homemade web browser in Rust | Rust | unknown | Low activity (23 commits) | 1 | Engine/Browser |
| 73 | ubugeeei/kurt | Toy browser: HTML/CSS parser, layout, renderer | Rust | MIT | Actively updated (180 commits, CI) | 1 | Engine/Browser |
| 74 | RobertBergman/rustbrowser | Browser using tao/wry webview | Rust | unknown | Inactive (1 commit) | 2 | Engine/Browser |
| 75 | EthanRStokes/stokes-browser | "A (bad) web browser" | Rust | unknown | Unclear (507 commits) | 1 | Engine/Browser |
| 76 | sidharthk9/RustBrowser | Miniature browser, tiny parsers | Rust | Unlicense | Low activity (16 commits) | 1 | Engine/Browser |
| 77 | Jason5Lee/rslide | Web service to navigate HTML pages as a slideshow | Rust | AGPL-3.0 | Low activity (4 commits) | 4 | Tangential |
| 78 | SkuldNorniern/celeris | High-performance ground-up browser engine | Rust | unknown | Low activity (47 commits) | 2 | Engine/Browser |
| 79 | theinfinityglitch/rusted-webkit | A simple browser engine | Rust | unknown | Very low (2 commits) | 1 | Engine/Browser |
| 80 | zduny/js-utils | Utilities for browser Rust applications | Rust | Apache-2.0 | Low/moderate (40 commits, CI) | 1 | Tangential |
| 81 | PeterWrighten/PetersBrowserEngine | Re-implementation of a web engine | Rust | unknown | Low activity (15 commits) | 1 | Engine/Browser |
| 82 | qrunk/Gemini | — | — | — | **Unreachable (404)** | — | Unreachable |
| 83 | cLazyZombie/web-browser-engineering-rs | Rust implementation of the Web Browser Engineering book | Rust | unknown | Very low (1 commit) | 1 | Engine/Browser |
| 84 | wowkster/bowser | Web browser written entirely in Rust | Rust (inferred) | unknown | Low activity (14 commits) | 1 | Engine/Browser |
| 85 | jpiggg/wave | A toy web browser rendering engine | Rust | unknown | Low activity (7 commits) | 1 | Engine/Browser |
| 86 | jdrouet/browser-crypto | Safe Rust interface to the Web Crypto API | Rust | MIT | Low/moderate (16 commits) | 2 | Unrelated |
| 87 | oza6ut0ne/rattice | Media viewer for web browsers | Rust | Apache-2.0/MIT | Moderate (132 commits, v0.6.1) | 3 | Unrelated |
| 88 | vats98754/rust-web-browser-from-scratch | Browser engine to learn browser/Chromium internals | Rust | unknown | Low activity (27 commits) | 1 | Engine/Browser |
| 89 | lapla-cogito/mercurius | A toy web browser written in Rust | Rust | MIT | Low activity (26 commits) | 1 | Engine/Browser |
| 90 | samziz/neutron | "An alternative Web, with browser included" | Rust (inferred) | unknown | Very early (3 commits, WIP) | 1 | Engine/Browser |
| 91 | kllarena07/qzgen | QR code generator usable via the browser | Rust (inferred) | unknown | Low activity (17 commits) | 1 | Unrelated |
| 92 | vedantwpatil/Rust-Toy-Web-Engine | Toy web engine, "Web Browser Engineering" book | Rust, Python | unknown | Low activity (52 commits) | 2 | Engine/Browser |
| 93 | babalolajnr/webwraith | Toy browser engine inspired by mbrubeck/robinson | Rust | unknown | Low activity (13 commits) | 1 | Engine/Browser |
| 94 | 0xlax/juul | Web browser built in Rust (toy project) | Rust (inferred) | unknown | Low activity (6 commits) | 1 | Engine/Browser |
| 95 | SinaBYR/astronaut | Simple web browser engine | Rust (inferred) | unknown | Low activity (19 commits) | 2 | Engine/Browser |
| 96 | aki-akaguma/webapp-broinfo | Webapp showing browser/hardware info | unknown | unknown | Low activity (45 commits) | 1 | Unrelated |
| 97 | SieloBrowser/sielo-core | Core module of Sielo Browser | Rust (inferred) | GPL-3.0 | Low activity (8 commits, 4 open issues) | 3 | Engine/Browser |
| 98 | OmChillure/webbrowser | Browser using TCP/IP listeners and thread pooling | Rust | unknown | Low activity (3 commits) | 2 | Engine/Browser |
| 99 | joinemm/rust-wasm-webgl | Rust-to-WASM 3D graphics demo for the browser | Rust, JS | unknown | Archived (2021, read-only) | 1 | Unrelated |
| 100 | sanagaya/minimal-rust-browser | Minimal browser: HTML/CSS parser, block layout, HTTP | Rust | unknown | Low activity (3 commits) | 1 | Engine/Browser |
| 101 | PhoenixFlame101/wasm-webgl-viewer | Viewer to run Rust graphics programs in-browser via WASM | unknown | AGPL-3.0 | Low activity (1 commit) | 1 | Unrelated |
| 102 | text-yoga/ask | Browser-based LLM "Ask" widget for websites | TypeScript | MIT | Low activity, early-stage (11 commits) | 2 | Unrelated |
| 103 | signalreason/lester-2026 | AI-first research browser: tagging, workspaces, privacy | Rust, TS | unknown | Low activity (5 commits) | 1 | Engine/Browser |
| 104 | hasnocool/rusty_wallhaven_browser | egui app to browse/download Wallhaven wallpapers | Rust | MIT | Low activity (5 commits) | 2 | Unrelated |
| 105 | billypearce/rust-web-chat | Browser-based chat app using Axum | Rust | unknown | Low activity (21 commits) | 1 | Unrelated |
| 106 | ThomasHabets/livecount | Widget: how many people have a page open right now | Rust | GPL-2.0 | Low activity (58 commits) | 9 | Unrelated |
| 107 | Hugo4IT/Growser | "A web browser made in Godot, just because I can" | Godot (not Rust-native) | unknown | Unclear | 2 | Engine/Browser |
| 108 | carlosmndzg/ferrum | Browser layout engine from scratch (thesis project) | Rust | unknown | Low activity (65 commits) | 2 | Engine/Browser |
| 109 | 46ki75/web-image-converter | In-browser image conversion via WASM | Rust | MIT | Low activity (76 commits) | 1 | Unrelated |
| 110 | elde-n/tridactyl-native | Native client for the Tridactyl browser extension | Rust | GPL-3.0 | Low activity (8 commits) | 2 | Tangential |
| 111 | rtleong/browser-engine | Rust rendering engine: HTML/CSS subset, DOM, selectors, layout | Rust | MIT | Low activity (41 commits) | 1 | Engine/Browser |
| 112 | loyal812/rust-webrtc | Simple WebRTC streaming server to a browser client | Rust | unknown | Low activity (6 commits) | 2 | Unrelated |
| 113 | alyque12/lightyear | Toy browser engine, first-time Rust project | Rust | unknown | Low activity (5 commits) | 1 | Engine/Browser |
| 114 | Hdpbilly/LuminaEngine | "WebGPU, wasm based web browser world engine with web3 and AI" | unknown | unknown | Unclear | 1 | Ambiguous |
| 115 | p2js/webless | "Web without the Web" — vague description | Rust | unknown | Low activity (17 commits) | 1 | Ambiguous |
| 116 | andrewabela/YetAnotherAIBrowser | Browser replacing HTML fetching with AI-generated content | Rust | MIT | Low activity (14 commits, dev branch) | 2 | Engine/Browser |
| 117 | mamidon/newt | "A web browser specifically for making desktop GUIs" | Rust (inferred) | unknown | Unclear | 2 | Engine/Browser |
| 118 | SignalWalker/Construct | Proof-of-concept browser for generic XML instead of HTML | unknown | unknown | Archived (Nov 2022) | 1 | Engine/Browser |
| 119 | igorsantos07/webapp-wrappers | Tauri wrappers to run web apps as standalone desktop apps | unknown (Tauri) | unknown | Low activity (7 commits) | 1 | Tangential |
| 120 | abghim/airbrowse | Minimal macOS browser: Rust + Slint UI + wry (WKWebView) | Rust | MIT | Low activity (26 commits) | 1 | Engine/Browser |
| 121 | lpfy/html2pdf-api | Thread-safe browser pool for HTML-to-PDF conversion | Rust | MIT | ~37 commits | 4 | Unrelated |
| 122 | clejacquet/rust-wgpu-raytracing | Prototype ray-tracer using WGPU in browsers | Rust | MIT | Unclear | 3 | Unrelated |
| 123 | saravenpi/calm | Minimalist privacy browser (WRY/TAO), fingerprinting protection | Rust | MIT | ~29 commits | 2 | Engine/Browser |
| 124 | funnyboy-roks/wannabe-ssh | Run pre-defined commands from a web browser | Rust | unknown | Single commit, early-stage | 1 | Unrelated |
| 125 | elyas-code/grob | Web browser engine aiming to be fast, efficient, modular | Rust | GPL-3.0 | Unclear | 2 | Engine/Browser |
| 126 | zero2null/web-beam | Real-time P2P data sharing between browsers | Rust | BSD-3-Clause | ~87 commits | 1 | Unrelated |
| 127 | Mjkli/ObscuraLink | Encryption tool for "securly browsing the web" | Rust | MIT | ~45 commits | 1 | Unrelated |
| 128 | LittleSaya/wasm-zip-stream | Stream-style zip tool running in browsers (WASM) | Rust | Apache-2.0 | ~7 commits | 2 | Unrelated |
| 129 | AlvinHon/webmessage | Rust/Wasm lib: signed/hashed messages in browser localStorage | Rust | Apache-2.0 | ~12 commits | 1 | Unrelated |
| 130 | Wormarz/justrans | Simple LAN file-exchange usable via a web browser | Rust | GPL-3.0 | ~31 commits | 1 | Unrelated |
| 131 | Dp5439/hyprbrowser | — | — | — | **Unreachable (404)** | — | Unreachable |
| 132 | keys-i/tinyserve | Ultra-lightweight HTTP file server for in-browser preview | Rust | BSD-3-Clause | ~6 commits | 1 | Unrelated |
| 133 | mateolafalce/DecenwserBlockchainProgram | Decentralized browser for anonymity/immutable info transmission | Rust | Apache-2.0 | ~3 commits, tiny | 1 | Ambiguous |
| 134 | juliog922/rust_yew_frontend | Yew UI demo: auth, routing, CRUD, state, fetch | Rust | unknown | Unclear | 1 | Unrelated |
| 135 | XertroV/discourse-rss-auto-archive-linked-content | Archives Discourse RSS links to S3 with a web search UI | Rust | MIT | 509 commits | 1 | Unrelated |

## Rough tally

- **Engine/Browser** (genuine browser or engine projects): ~78
- **Tangential** (browser-adjacent tooling, not itself a browser): ~19
- **Unrelated**: ~34
- **Unreachable** (404): 2
- **Ambiguous** (needs a manual look): 4 — `mariluski/reSearch`,
  `Hdpbilly/LuminaEngine`, `p2js/webless`, `mateolafalce/DecenwserBlockchainProgram`

Roughly 58% of the list is a genuine browser/engine project matching the
original premise; the rest is either browser-adjacent tooling or unrelated
stars pulled in by keyword overlap. See ARCHITECTURE.md for how the two
integration candidates found so far (gosub-engine, evaluated against the
already-abandoned OkuBrowser AGPL blocker) were chosen from this set.
