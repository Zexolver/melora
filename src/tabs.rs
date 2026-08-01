use std::collections::VecDeque;
use std::sync::Arc;

use blitz_dom::net::Resource;
use blitz_traits::net::NetProvider;

use crate::engine::PageEngine;
use crate::session::SessionTab;
use crate::swap::{SwapFile, SwapSlot};

pub type TabId = u64;

/// How many compressed tabs are allowed to keep their LZ4 bytes in RAM
/// before the coldest ones get spilled to `TabManager::swap` instead --
/// the disk-backed tier underneath the RAM one. 32 is a lot more than
/// `max_active` (so recently-backgrounded tabs stay in fast RAM) while
/// still bounding RAM use for sessions with hundreds of tabs open. See
/// `TabManager::set_ram_compressed_budget` for overriding it (tests only
/// -- real usage always wants the default).
const DEFAULT_RAM_COMPRESSED_BUDGET: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TabState {
    Active,
    /// LZ4-compressed source HTML held in RAM.
    Compressed,
    /// LZ4-compressed source HTML written to `TabManager::swap` (disk)
    /// instead of kept in RAM -- the coldest tier.
    Swapped,
}

/// Outcome of trying to wake a tab. See [`TabManager::activate`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WakeResult {
    /// Was already active; just marked most-recently-used.
    AlreadyActive,
    /// Was compressed in RAM; decompressed and re-parsed locally, no
    /// network needed.
    WokeFromCompressed,
    /// Was compressed on disk (the swap tier); read back, decompressed,
    /// and re-parsed locally -- still no network needed, just a little
    /// disk I/O the RAM-only case doesn't pay.
    WokeFromSwap,
    /// No local snapshot exists (shouldn't happen for a tab that has ever
    /// successfully loaded something) -- caller must fetch over the
    /// network and apply it with `TabManager::force_activate`.
    NeedsRefetch,
}

pub struct Tab {
    pub id: TabId,
    pub url: String,
    pub title: String,
    pub state: TabState,
    history: Vec<String>,
    history_pos: usize,
    engine: Option<PageEngine>,
    /// Raw bytes `engine` was built from. Kept alongside a live engine so
    /// a later demotion can compress them without a network round-trip;
    /// cleared once demoted (`compressed_html` becomes the sole
    /// representation of the tab's content).
    source_html: Option<Vec<u8>>,
    /// LZ4-compressed HTML, present only while `state == Compressed`.
    /// Decompressing and re-parsing this is the whole point of this
    /// hibernation tier: waking a tab needs no network round-trip.
    compressed_html: Option<Vec<u8>>,
    /// Where this tab's LZ4-compressed HTML lives in `TabManager::swap`,
    /// present only while `state == Swapped`.
    swap_slot: Option<SwapSlot>,
}

impl Tab {
    pub fn node_count(&self) -> usize {
        self.engine.as_ref().map(PageEngine::node_count).unwrap_or(0)
    }

    /// Size in bytes of this tab's compressed snapshot; 0 while active (no
    /// snapshot needed) or before anything has ever loaded.
    pub fn compressed_bytes(&self) -> usize {
        self.compressed_html.as_ref().map(Vec::len).unwrap_or(0)
    }

    /// Rasterized RGBA8 pixels of the page as currently scrolled, or
    /// `None` if this tab has no live engine (compressed, or never loaded)
    /// or painting hit a contained internal panic (see
    /// `PageEngine::paint`).
    pub fn paint(&self) -> Option<Vec<u8>> {
        self.engine.as_ref().and_then(PageEngine::paint)
    }

    /// Scrolls the page; a no-op if this tab has no live engine.
    pub fn scroll_by(&mut self, dx: f64, dy: f64) {
        if let Some(engine) = self.engine.as_mut() {
            engine.scroll_by(dx, dy);
        }
    }

    /// This tab's document id, used to route an incoming `Resource` to the
    /// right tab. `None` if there's no live engine.
    pub fn doc_id(&self) -> Option<usize> {
        self.engine.as_ref().map(PageEngine::doc_id)
    }

    /// Loads a fetched sub-resource without re-resolving layout -- see
    /// `TabManager::load_resource`/`resolve_layout_for_doc` for why a
    /// caller applying a whole batch of resources to this tab in one go
    /// wants that as a separate, single step rather than paying for a
    /// full layout pass per resource.
    pub fn load_resource(&mut self, resource: Resource) -> bool {
        self.engine.as_mut().is_some_and(|engine| engine.load_resource(resource))
    }

    /// Just the layout-resolve half -- see `load_resource` above.
    pub fn resolve_layout(&mut self) -> bool {
        self.engine.as_mut().is_some_and(|engine| engine.resolve_layout())
    }

    /// If `(x, y)` lands on a link, the URL it resolves to. `None` if
    /// there's no live engine or nothing was hit.
    pub fn hit_test_href(&self, x: f32, y: f32) -> Option<String> {
        self.engine.as_ref()?.hit_test_href(x, y)
    }
}

/// Owns every open tab and decides which ones keep a live [`PageEngine`]
/// resident in memory.
///
/// Only the `max_active` most recently used tabs stay "warm" (a live parsed
/// DOM + style + layout tree). Every other tab is demoted to an
/// LZ4-compressed copy of its source HTML in RAM -- a zram-style tier,
/// not a discard-and-refetch one: waking a compressed tab decompresses
/// and re-parses locally, with no network round-trip and no dependency on
/// being online. This is what lets a session hold hundreds of open tabs
/// without hundreds of live engines in memory, while still restoring each
/// one instantly on demand.
pub struct TabManager {
    tabs: Vec<Tab>,
    lru: VecDeque<TabId>, // front = least recently used, back = most recently used
    next_id: TabId,
    max_active: usize,
    viewport: (u32, u32),
    /// Lets every tab's document fetch its own sub-resources (stylesheets,
    /// images, fonts). `None` (the default -- see `new`) means pages parse
    /// fine but render with only inline/UA styles; set once via
    /// `set_resource_provider` in the real app. Left unset in tests, which
    /// use self-contained HTML with no sub-resources to fetch.
    resource_provider: Option<Arc<dyn NetProvider<Resource>>>,
    /// The disk-backed overflow tier underneath the RAM-compressed one --
    /// see `TabState::Swapped` and `swap.rs`.
    swap: SwapFile,
    ram_compressed_budget: usize,
}

impl TabManager {
    pub fn new(max_active: usize, viewport: (u32, u32)) -> Self {
        Self {
            tabs: Vec::new(),
            lru: VecDeque::new(),
            next_id: 0,
            max_active,
            viewport,
            resource_provider: None,
            swap: SwapFile::new().expect("failed to create melora's swap file"),
            ram_compressed_budget: DEFAULT_RAM_COMPRESSED_BUDGET,
        }
    }

    pub fn set_resource_provider(&mut self, provider: Arc<dyn NetProvider<Resource>>) {
        self.resource_provider = Some(provider);
    }

    /// Overrides how many compressed tabs may stay in RAM before the
    /// coldest are spilled to disk. Test-only: real usage always wants
    /// `DEFAULT_RAM_COMPRESSED_BUDGET`, but exercising the swap tier in a
    /// test without opening dozens of tabs needs a much smaller budget.
    #[cfg(test)]
    pub(crate) fn set_ram_compressed_budget(&mut self, budget: usize) {
        self.ram_compressed_budget = budget;
    }

    pub fn viewport(&self) -> (u32, u32) {
        self.viewport
    }

    pub fn open_tab(&mut self, url: impl Into<String>, html: &str) -> TabId {
        let id = self.next_id;
        self.next_id += 1;
        let url = url.into();
        let engine = PageEngine::from_html(html, &url, self.viewport, self.resource_provider.clone());
        let title = engine.title_override().map(str::to_string).unwrap_or_else(|| url.clone());
        self.tabs.push(Tab {
            id,
            title,
            url: url.clone(),
            state: TabState::Active,
            history: vec![url],
            history_pos: 0,
            engine: Some(engine),
            source_html: Some(html.as_bytes().to_vec()),
            compressed_html: None,
            swap_slot: None,
        });
        self.lru.push_back(id);
        self.enforce_budget();
        id
    }

    pub fn close_tab(&mut self, id: TabId) {
        self.tabs.retain(|t| t.id != id);
        self.lru.retain(|&t| t != id);
    }

    /// Load `html` into `id` without touching browsing history. Used for
    /// reload, back/forward, and the network-refetch fallback path.
    fn load(&mut self, id: TabId, url: &str, html: &str) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            let engine = PageEngine::from_html(html, url, self.viewport, self.resource_provider.clone());
            tab.title = engine.title_override().map(str::to_string).unwrap_or_else(|| url.to_string());
            tab.url = url.to_string();
            tab.engine = Some(engine);
            tab.source_html = Some(html.as_bytes().to_vec());
            tab.compressed_html = None;
            tab.swap_slot = None;
            tab.state = TabState::Active;
        }
        self.lru.retain(|&t| t != id);
        self.lru.push_back(id);
        self.enforce_budget();
    }

    /// Wakes a tab using its local compressed snapshot, with no network
    /// involved -- see `WakeResult`. Also marks it most-recently-used.
    pub fn activate(&mut self, id: TabId) -> WakeResult {
        let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) else {
            return WakeResult::NeedsRefetch;
        };

        if tab.state == TabState::Active {
            self.lru.retain(|&t| t != id);
            self.lru.push_back(id);
            return WakeResult::AlreadyActive;
        }

        let url = tab.url.clone();
        let state = tab.state;
        let ram_bytes = tab.compressed_html.take();
        let swap_slot = tab.swap_slot.take();

        let compressed = match (ram_bytes, swap_slot) {
            (Some(bytes), _) => bytes,
            (None, Some(slot)) => {
                let Ok(bytes) = self.swap.read(slot) else {
                    return WakeResult::NeedsRefetch;
                };
                bytes
            }
            (None, None) => return WakeResult::NeedsRefetch,
        };
        let Ok(bytes) = lz4_flex::decompress_size_prepended(&compressed) else {
            return WakeResult::NeedsRefetch;
        };
        let html = String::from_utf8_lossy(&bytes).into_owned();
        let engine = PageEngine::from_html(&html, &url, self.viewport, self.resource_provider.clone());
        let title = engine.title_override().map(str::to_string).unwrap_or(url);

        let tab = self.tabs.iter_mut().find(|t| t.id == id).unwrap();
        tab.title = title;
        tab.engine = Some(engine);
        tab.source_html = Some(bytes);
        tab.state = TabState::Active;

        self.lru.retain(|&t| t != id);
        self.lru.push_back(id);
        self.enforce_budget();

        if state == TabState::Swapped {
            WakeResult::WokeFromSwap
        } else {
            WakeResult::WokeFromCompressed
        }
    }

    /// Escape hatch for `WakeResult::NeedsRefetch`: force-loads freshly
    /// fetched `html` and marks the tab active, without touching history.
    pub fn force_activate(&mut self, id: TabId, html: &str) {
        let url = self.tab(id).map(|t| t.url.clone()).unwrap_or_default();
        self.load(id, &url, html);
    }

    /// User-initiated navigation: loads new content and pushes it onto the
    /// tab's back/forward history.
    pub fn navigate(&mut self, id: TabId, url: impl Into<String>, html: &str) {
        let url = url.into();
        self.load(id, &url, html);
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.history.truncate(tab.history_pos + 1);
            tab.history.push(url);
            tab.history_pos = tab.history.len() - 1;
        }
    }

    pub fn reload(&mut self, id: TabId, html: &str) {
        if let Some(url) = self.tabs.iter().find(|t| t.id == id).map(|t| t.url.clone()) {
            self.load(id, &url, html);
        }
    }

    pub fn can_go_back(&self, id: TabId) -> bool {
        self.tabs
            .iter()
            .find(|t| t.id == id)
            .is_some_and(|t| t.history_pos > 0)
    }

    pub fn can_go_forward(&self, id: TabId) -> bool {
        self.tabs
            .iter()
            .find(|t| t.id == id)
            .is_some_and(|t| t.history_pos + 1 < t.history.len())
    }

    /// The URL `go_back` would load, without applying anything. Lets a
    /// caller fetch that URL's content first (e.g. over the network) and
    /// only then call `go_back` with the result in hand.
    pub fn peek_back_url(&self, id: TabId) -> Option<String> {
        self.tabs.iter().find(|t| t.id == id).and_then(|t| {
            (t.history_pos > 0).then(|| t.history[t.history_pos - 1].clone())
        })
    }

    /// The URL `go_forward` would load, without applying anything. See
    /// `peek_back_url`.
    pub fn peek_forward_url(&self, id: TabId) -> Option<String> {
        self.tabs.iter().find(|t| t.id == id).and_then(|t| {
            (t.history_pos + 1 < t.history.len()).then(|| t.history[t.history_pos + 1].clone())
        })
    }

    /// Scrolls the given tab's page, if it has a live engine. See
    /// `PageEngine::scroll_by` for the sign convention.
    pub fn scroll_active(&mut self, id: TabId, dx: f64, dy: f64) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.scroll_by(dx, dy);
        }
    }

    pub fn go_back(&mut self, id: TabId, html: &str) {
        let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) else {
            return;
        };
        if tab.history_pos == 0 {
            return;
        }
        tab.history_pos -= 1;
        let url = tab.history[tab.history_pos].clone();
        self.load(id, &url, html);
    }

    pub fn go_forward(&mut self, id: TabId, html: &str) {
        let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) else {
            return;
        };
        if tab.history_pos + 1 >= tab.history.len() {
            return;
        }
        tab.history_pos += 1;
        let url = tab.history[tab.history_pos].clone();
        self.load(id, &url, html);
    }

    /// Routes a fetched sub-resource to whichever tab's document it
    /// belongs to (matched by doc id, since resources arrive asynchronously
    /// and the tab may since have navigated away, been closed, or been
    /// demoted), loading it without a layout re-resolve. Returns `true` if
    /// a live tab was found and updated. Pairs with `resolve_layout_for_doc`
    /// below, meant to be called once after every resource in a batch
    /// destined for the same document has been loaded, rather than once
    /// per resource. `lib.rs`'s network-draining timer is the real caller:
    /// it can receive several resources for the same page in one 50ms tick
    /// (a real page with several stylesheets, like duckduckgo.com's 8, all
    /// arriving close together), and resolving layout after each one
    /// individually means doing a full synchronous layout pass on Slint's
    /// UI thread once per resource instead of once per batch -- confirmed
    /// live to be slow enough to trip Android's ANR watchdog.
    pub fn load_resource(&mut self, doc_id: usize, resource: Resource) -> bool {
        let Some(tab) = self.tabs.iter_mut().find(|t| t.doc_id() == Some(doc_id)) else {
            return false;
        };
        tab.load_resource(resource);
        true
    }

    /// The other half of `load_resource` above.
    pub fn resolve_layout_for_doc(&mut self, doc_id: usize) -> bool {
        let Some(tab) = self.tabs.iter_mut().find(|t| t.doc_id() == Some(doc_id)) else {
            return false;
        };
        tab.resolve_layout()
    }

    /// Re-lays-out every currently-active tab for a new viewport size
    /// (e.g. the content pane was resized) and remembers it for tabs
    /// created or woken afterward.
    pub fn resize_viewport(&mut self, width: u32, height: u32) {
        self.viewport = (width, height);
        for tab in self.tabs.iter_mut() {
            if let Some(engine) = tab.engine.as_mut() {
                engine.resize(width, height);
            }
        }
    }

    /// Demotes the least-recently-used active tabs beyond `max_active` to
    /// the compressed tier (their engine is dropped and their source HTML
    /// is LZ4-compressed in its place), then, if that leaves more than
    /// `ram_compressed_budget` tabs holding compressed bytes in RAM,
    /// spills the coldest of those to the disk-backed swap tier instead.
    fn enforce_budget(&mut self) {
        let active_count = self.tabs.iter().filter(|t| t.state == TabState::Active).count();
        if active_count > self.max_active {
            let mut to_demote = active_count - self.max_active;
            for id in self.lru.iter() {
                if to_demote == 0 {
                    break;
                }
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == *id) {
                    if tab.state == TabState::Active {
                        if let Some(bytes) = tab.source_html.take() {
                            tab.compressed_html = Some(lz4_flex::compress_prepend_size(&bytes));
                        }
                        tab.engine = None;
                        tab.state = TabState::Compressed;
                        to_demote -= 1;
                    }
                }
            }
        }

        let ram_compressed_count = self.tabs.iter().filter(|t| t.state == TabState::Compressed).count();
        if ram_compressed_count <= self.ram_compressed_budget {
            return;
        }
        let mut to_swap = ram_compressed_count - self.ram_compressed_budget;
        for id in self.lru.iter() {
            if to_swap == 0 {
                break;
            }
            if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == *id) {
                if tab.state == TabState::Compressed {
                    if let Some(bytes) = tab.compressed_html.take() {
                        match self.swap.write(&bytes) {
                            Ok(slot) => {
                                tab.swap_slot = Some(slot);
                                tab.state = TabState::Swapped;
                                to_swap -= 1;
                            }
                            Err(_) => {
                                // Disk write failed (e.g. no space left) --
                                // keep the tab's bytes in RAM rather than
                                // lose them.
                                tab.compressed_html = Some(bytes);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    pub fn tab(&self, id: TabId) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    pub fn active_count(&self) -> usize {
        self.tabs.iter().filter(|t| t.state == TabState::Active).count()
    }

    pub fn compressed_count(&self) -> usize {
        self.tabs.iter().filter(|t| t.state == TabState::Compressed).count()
    }

    pub fn swapped_count(&self) -> usize {
        self.tabs.iter().filter(|t| t.state == TabState::Swapped).count()
    }

    /// Total bytes currently held in compressed snapshots in RAM, across
    /// every compressed tab. A concrete, computed measure of what the
    /// "hundreds of tabs, very little RAM" claim actually costs.
    pub fn total_compressed_bytes(&self) -> usize {
        self.tabs.iter().map(Tab::compressed_bytes).sum()
    }

    /// Total bytes currently held in the disk-backed swap tier. Read
    /// straight from each tab's slot length, no disk I/O needed.
    pub fn total_swapped_bytes(&self) -> usize {
        self.tabs.iter().filter_map(|t| t.swap_slot).map(|s| s.len as usize).sum()
    }

    /// Builds a session snapshot of every open tab -- enough to fully
    /// reconstruct each one (LZ4-compressed HTML plus url/title/history)
    /// regardless of which tier it's currently in, including reading any
    /// swapped-to-disk tabs back in. `active_id` marks which tab was on
    /// screen, so a restore can bring the same one back to the front.
    pub fn session_snapshot(&mut self, active_id: Option<TabId>) -> Vec<SessionTab> {
        let mut out = Vec::with_capacity(self.tabs.len());
        for i in 0..self.tabs.len() {
            let tab = &self.tabs[i];
            let id = tab.id;
            let url = tab.url.clone();
            let title = tab.title.clone();
            let history = tab.history.clone();
            let history_pos = tab.history_pos;
            let state = tab.state;
            let ram_bytes = tab.compressed_html.clone();
            let swap_slot = tab.swap_slot;
            let source_html = tab.source_html.clone();

            let compressed_html = match state {
                TabState::Active => source_html.map(|bytes| lz4_flex::compress_prepend_size(&bytes)),
                TabState::Compressed => ram_bytes,
                TabState::Swapped => swap_slot.and_then(|slot| self.swap.read(slot).ok()),
            };

            let Some(compressed_html) = compressed_html else { continue };
            out.push(SessionTab {
                url,
                title,
                history,
                history_pos,
                active: Some(id) == active_id,
                compressed_html,
            });
        }
        out
    }

    /// Reconstructs tabs from a saved session, in the compressed tier (no
    /// live engine parsed yet -- restoring a session with hundreds of
    /// tabs shouldn't parse and lay out all of them up front). Returns
    /// the id of whichever tab was marked active when the session was
    /// saved, if any; the caller should `activate` it to bring it live.
    pub fn restore_session(&mut self, tabs: Vec<SessionTab>) -> Option<TabId> {
        let mut restored_active = None;
        for t in tabs {
            let id = self.next_id;
            self.next_id += 1;
            if t.active {
                restored_active = Some(id);
            }
            self.tabs.push(Tab {
                id,
                title: t.title,
                url: t.url,
                state: TabState::Compressed,
                history: t.history,
                history_pos: t.history_pos,
                engine: None,
                source_html: None,
                compressed_html: Some(t.compressed_html),
                swap_slot: None,
            });
            self.lru.push_back(id);
        }
        self.enforce_budget();
        restored_active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEMO: &str = "<html><body><p>demo</p></body></html>";

    /// Real-ish page: long enough for LZ4 to have something to compress,
    /// unlike the few-byte DEMO string used everywhere else in these tests.
    fn repetitive_page(paragraphs: usize) -> String {
        let mut html = String::from("<html><body>");
        for i in 0..paragraphs {
            html.push_str(&format!(
                "<p>This is paragraph number {i} of a fairly ordinary web page, \
                 with the kind of repeated boilerplate real HTML actually has.</p>"
            ));
        }
        html.push_str("</body></html>");
        html
    }

    #[test]
    fn opening_tabs_beyond_budget_compresses_the_oldest() {
        let mut mgr = TabManager::new(2, (800, 600));
        let a = mgr.open_tab("a", DEMO);
        let b = mgr.open_tab("b", DEMO);
        let c = mgr.open_tab("c", DEMO);

        assert_eq!(mgr.active_count(), 2);
        assert_eq!(mgr.compressed_count(), 1);
        assert_eq!(mgr.tab(a).unwrap().state, TabState::Compressed);
        assert_eq!(mgr.tab(b).unwrap().state, TabState::Active);
        assert_eq!(mgr.tab(c).unwrap().state, TabState::Active);
    }

    #[test]
    fn demoting_a_tab_actually_compresses_its_content() {
        let mut mgr = TabManager::new(1, (800, 600));
        let page = repetitive_page(50);
        let a = mgr.open_tab("a", &page);
        mgr.open_tab("b", DEMO); // demotes a

        assert_eq!(mgr.tab(a).unwrap().state, TabState::Compressed);
        let compressed_len = mgr.tab(a).unwrap().compressed_bytes();
        assert!(compressed_len > 0, "expected a non-empty compressed snapshot");
        assert!(
            compressed_len < page.len(),
            "expected real compression: {compressed_len} bytes compressed vs {} original",
            page.len()
        );
    }

    #[test]
    fn waking_a_compressed_tab_needs_no_externally_supplied_html() {
        let mut mgr = TabManager::new(1, (800, 600));
        let page = repetitive_page(50);
        let a = mgr.open_tab("a", &page);
        let original_node_count = mgr.tab(a).unwrap().node_count();
        mgr.open_tab("b", DEMO); // demotes a
        assert_eq!(mgr.tab(a).unwrap().state, TabState::Compressed);

        let result = mgr.activate(a);

        assert_eq!(result, WakeResult::WokeFromCompressed);
        assert_eq!(mgr.tab(a).unwrap().state, TabState::Active);
        assert_eq!(mgr.tab(a).unwrap().node_count(), original_node_count);
        // Waking demoted the LRU tab (b) in a's place, since the budget is 1.
        assert_eq!(mgr.tab(mgr.tabs()[1].id).unwrap().state, TabState::Compressed);
    }

    #[test]
    fn activating_an_already_active_tab_is_a_touch_not_a_reload() {
        let mut mgr = TabManager::new(4, (800, 600));
        let a = mgr.open_tab("a", DEMO);
        assert_eq!(mgr.activate(a), WakeResult::AlreadyActive);
        assert_eq!(mgr.tab(a).unwrap().state, TabState::Active);
    }

    #[test]
    fn closing_a_tab_removes_it_entirely() {
        let mut mgr = TabManager::new(4, (800, 600));
        let a = mgr.open_tab("a", DEMO);
        mgr.close_tab(a);
        assert!(mgr.tab(a).is_none());
        assert_eq!(mgr.tabs().len(), 0);
    }

    #[test]
    fn hundreds_of_tabs_stay_within_the_active_budget_and_compressed_total_stays_small() {
        let mut mgr = TabManager::new(8, (800, 600));
        let page = repetitive_page(20);
        for i in 0..300 {
            mgr.open_tab(format!("tab-{i}"), &page);
        }
        assert_eq!(mgr.tabs().len(), 300);
        assert_eq!(mgr.active_count(), 8);
        // The 292 non-active tabs split across the two hibernation tiers:
        // the most recently backgrounded stay RAM-compressed up to the
        // budget, the rest spill to the disk swap tier underneath it.
        assert_eq!(mgr.compressed_count() + mgr.swapped_count(), 292);
        assert!(mgr.compressed_count() <= 32);
        assert!(mgr.swapped_count() > 0, "300 tabs should be enough to exercise the swap tier too");

        // 292 compressed tabs of a ~1.6KB page should total well under 1MB
        // combined across both tiers, not 292 live engines' worth of
        // DOM/style/layout state.
        let total = mgr.total_compressed_bytes() + mgr.total_swapped_bytes();
        assert!(total > 0);
        assert!(
            total < page.len() * 292,
            "compressed total ({total} bytes) should be far less than storing \
             292 uncompressed copies ({} bytes)",
            page.len() * 292
        );
    }

    #[test]
    fn back_and_forward_walk_history_without_growing_it() {
        let mut mgr = TabManager::new(4, (800, 600));
        let a = mgr.open_tab("a", DEMO);
        mgr.navigate(a, "b", DEMO);
        mgr.navigate(a, "c", DEMO);
        assert_eq!(mgr.tab(a).unwrap().url, "c");

        mgr.go_back(a, DEMO);
        assert_eq!(mgr.tab(a).unwrap().url, "b");
        assert!(mgr.can_go_back(a));
        assert!(mgr.can_go_forward(a));

        mgr.go_back(a, DEMO);
        assert_eq!(mgr.tab(a).unwrap().url, "a");
        assert!(!mgr.can_go_back(a));

        mgr.go_forward(a, DEMO);
        assert_eq!(mgr.tab(a).unwrap().url, "b");
    }

    #[test]
    fn peek_back_and_forward_report_the_target_without_mutating_state() {
        let mut mgr = TabManager::new(4, (800, 600));
        let a = mgr.open_tab("a", DEMO);
        mgr.navigate(a, "b", DEMO);
        mgr.navigate(a, "c", DEMO);

        assert_eq!(mgr.peek_back_url(a).as_deref(), Some("b"));
        assert_eq!(mgr.peek_forward_url(a), None);
        // Peeking must not have moved history_pos.
        assert_eq!(mgr.tab(a).unwrap().url, "c");

        mgr.go_back(a, DEMO);
        assert_eq!(mgr.peek_back_url(a).as_deref(), Some("a"));
        assert_eq!(mgr.peek_forward_url(a).as_deref(), Some("c"));
    }

    #[test]
    fn navigating_after_going_back_truncates_forward_history() {
        let mut mgr = TabManager::new(4, (800, 600));
        let a = mgr.open_tab("a", DEMO);
        mgr.navigate(a, "b", DEMO);
        mgr.go_back(a, DEMO);
        mgr.navigate(a, "z", DEMO);

        assert!(!mgr.can_go_forward(a));
        assert_eq!(mgr.tab(a).unwrap().url, "z");
    }

    #[test]
    fn resizing_the_viewport_relayouts_active_tabs_and_is_remembered_for_new_ones() {
        let mut mgr = TabManager::new(4, (800, 600));
        let a = mgr.open_tab("a", DEMO);
        assert_eq!(mgr.tab(a).unwrap().paint().unwrap().len(), 800 * 600 * 4);

        mgr.resize_viewport(400, 300);

        assert_eq!(mgr.tab(a).unwrap().paint().unwrap().len(), 400 * 300 * 4);
        assert_eq!(mgr.viewport(), (400, 300));

        // A tab opened after the resize should use the new size too.
        let b = mgr.open_tab("b", DEMO);
        assert_eq!(mgr.tab(b).unwrap().paint().unwrap().len(), 400 * 300 * 4);
    }

    /// `load_resource` routes a fetched sub-resource to whichever tab's
    /// document it belongs to (matched by doc id, since resources arrive
    /// asynchronously and the tab may since have navigated away, been
    /// closed, or been demoted); `resolve_layout_for_doc` is the paired
    /// layout re-resolve, meant to be called once after a whole batch of
    /// `load_resource` calls for the same document rather than once per
    /// resource -- see their doc comments for the real ANR on
    /// duckduckgo.com this split fixed. Both return `false` for a stale
    /// doc id, `true` once a live tab is found and updated.
    #[test]
    fn load_resource_and_resolve_layout_for_doc_route_by_doc_id_and_ignore_stale_ids() {
        let mut mgr = TabManager::new(4, (800, 600));
        let a = mgr.open_tab("a", DEMO);
        let doc_id = mgr.tab(a).unwrap().doc_id().unwrap();

        assert!(!mgr.load_resource(doc_id + 12345, Resource::None));
        assert!(!mgr.resolve_layout_for_doc(doc_id + 12345));

        assert!(mgr.load_resource(doc_id, Resource::None));
        assert!(mgr.resolve_layout_for_doc(doc_id));
    }

    #[test]
    fn compressed_tabs_beyond_the_ram_budget_spill_to_the_disk_swap_tier() {
        let mut mgr = TabManager::new(1, (800, 600));
        mgr.set_ram_compressed_budget(2);
        let page = repetitive_page(20);

        let a = mgr.open_tab("a", &page); // demoted to Compressed by b
        mgr.open_tab("b", DEMO);
        mgr.open_tab("c", DEMO); // demotes b to Compressed -- ram budget (2) not yet exceeded
        mgr.open_tab("d", DEMO); // demotes c -- 3 compressed now, budget 2: a (LRU) spills to swap

        assert_eq!(mgr.tab(a).unwrap().state, TabState::Swapped);
        assert_eq!(mgr.tab(a).unwrap().compressed_bytes(), 0, "swapped tabs hold no RAM copy");
        assert!(mgr.total_swapped_bytes() > 0);
        assert_eq!(mgr.swapped_count(), 1);
    }

    #[test]
    fn waking_a_swapped_tab_reads_it_back_from_disk() {
        let mut mgr = TabManager::new(1, (800, 600));
        mgr.set_ram_compressed_budget(1);
        let page = repetitive_page(20);

        let a = mgr.open_tab("a", &page);
        let original_node_count = mgr.tab(a).unwrap().node_count();
        mgr.open_tab("b", DEMO); // demotes a to Compressed
        mgr.open_tab("c", DEMO); // 2 compressed (a, b) > budget 1: a spills to swap
        assert_eq!(mgr.tab(a).unwrap().state, TabState::Swapped);

        let result = mgr.activate(a);

        assert_eq!(result, WakeResult::WokeFromSwap);
        assert_eq!(mgr.tab(a).unwrap().state, TabState::Active);
        assert_eq!(mgr.tab(a).unwrap().node_count(), original_node_count);
    }

    #[test]
    fn session_snapshot_and_restore_round_trips_url_title_history_and_content() {
        let mut mgr = TabManager::new(4, (800, 600));
        let a = mgr.open_tab("a", DEMO);
        mgr.navigate(a, "a/2", DEMO);
        let original_node_count = mgr.tab(a).unwrap().node_count();

        let snapshot = mgr.session_snapshot(Some(a));
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].url, "a/2");
        assert_eq!(snapshot[0].history, vec!["a".to_string(), "a/2".to_string()]);
        assert_eq!(snapshot[0].history_pos, 1);
        assert!(snapshot[0].active);

        let mut restored = TabManager::new(4, (800, 600));
        let active = restored.restore_session(snapshot);
        assert_eq!(restored.tabs().len(), 1);
        let restored_id = restored.tabs()[0].id;
        assert_eq!(active, Some(restored_id));
        assert_eq!(restored.tab(restored_id).unwrap().state, TabState::Compressed);

        // Restored tabs start compressed (no engine parsed yet); waking
        // the previously-active one should reproduce the same page.
        assert_eq!(restored.activate(restored_id), WakeResult::WokeFromCompressed);
        assert_eq!(restored.tab(restored_id).unwrap().node_count(), original_node_count);
        assert_eq!(restored.tab(restored_id).unwrap().url, "a/2");
    }

    #[test]
    fn session_snapshot_includes_swapped_tabs_by_reading_them_back_from_disk() {
        let mut mgr = TabManager::new(1, (800, 600));
        mgr.set_ram_compressed_budget(0);
        let page = repetitive_page(20);
        let a = mgr.open_tab("a", &page);
        mgr.open_tab("b", DEMO); // demotes a; ram budget 0 immediately spills it to swap
        assert_eq!(mgr.tab(a).unwrap().state, TabState::Swapped);

        let snapshot = mgr.session_snapshot(None);
        let a_snapshot = snapshot.iter().find(|t| t.url == "a").unwrap();
        assert!(!a_snapshot.compressed_html.is_empty());
        assert_eq!(
            lz4_flex::decompress_size_prepended(&a_snapshot.compressed_html).unwrap(),
            page.as_bytes()
        );
    }
}
