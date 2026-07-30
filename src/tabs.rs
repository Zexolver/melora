use std::collections::VecDeque;
use std::sync::Arc;

use blitz_dom::net::Resource;
use blitz_traits::net::NetProvider;

use crate::engine::PageEngine;

pub type TabId = u64;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TabState {
    Active,
    Compressed,
}

/// Outcome of trying to wake a tab. See [`TabManager::activate`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WakeResult {
    /// Was already active; just marked most-recently-used.
    AlreadyActive,
    /// Was compressed; decompressed and re-parsed locally, no network
    /// needed.
    WokeFromCompressed,
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

    /// Applies a fetched sub-resource; a no-op if this tab has no live
    /// engine (e.g. it was demoted between the fetch starting and
    /// finishing).
    pub fn apply_resource(&mut self, resource: Resource) {
        if let Some(engine) = self.engine.as_mut() {
            engine.apply_resource(resource);
        }
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
        }
    }

    pub fn set_resource_provider(&mut self, provider: Arc<dyn NetProvider<Resource>>) {
        self.resource_provider = Some(provider);
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
        let Some(compressed) = tab.compressed_html.take() else {
            return WakeResult::NeedsRefetch;
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

        WakeResult::WokeFromCompressed
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
    /// demoted). Returns `true` if a live tab was found and updated.
    pub fn apply_resource(&mut self, doc_id: usize, resource: Resource) -> bool {
        let Some(tab) = self.tabs.iter_mut().find(|t| t.doc_id() == Some(doc_id)) else {
            return false;
        };
        tab.apply_resource(resource);
        true
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
    /// the compressed tier: their engine is dropped and their source HTML
    /// is LZ4-compressed in its place.
    fn enforce_budget(&mut self) {
        let active_count = self.tabs.iter().filter(|t| t.state == TabState::Active).count();
        if active_count <= self.max_active {
            return;
        }
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

    /// Total bytes currently held in compressed snapshots, across every
    /// compressed tab. A concrete, computed measure of what the
    /// "hundreds of tabs, very little RAM" claim actually costs.
    pub fn total_compressed_bytes(&self) -> usize {
        self.tabs.iter().map(Tab::compressed_bytes).sum()
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
        assert_eq!(mgr.compressed_count(), 292);

        // 292 compressed tabs of a ~1.6KB page should total well under 1MB,
        // not 292 live engines' worth of DOM/style/layout state.
        let total = mgr.total_compressed_bytes();
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

    #[test]
    fn apply_resource_routes_to_the_matching_tab_by_doc_id_and_ignores_stale_ids() {
        let mut mgr = TabManager::new(4, (800, 600));
        let a = mgr.open_tab("a", DEMO);
        let doc_id = mgr.tab(a).unwrap().doc_id().unwrap();

        assert!(!mgr.apply_resource(doc_id + 12345, Resource::None));
        assert!(mgr.apply_resource(doc_id, Resource::None));
    }
}
