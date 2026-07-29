use std::collections::VecDeque;

use crate::engine::PageEngine;

pub type TabId = u64;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TabState {
    Active,
    Hibernated,
}

pub struct Tab {
    pub id: TabId,
    pub url: String,
    pub title: String,
    pub state: TabState,
    history: Vec<String>,
    history_pos: usize,
    engine: Option<PageEngine>,
}

impl Tab {
    pub fn node_count(&self) -> usize {
        self.engine.as_ref().map(PageEngine::node_count).unwrap_or(0)
    }
}

/// Owns every open tab and decides which ones keep a live [`PageEngine`]
/// resident in memory.
///
/// Only the `max_active` most recently used tabs stay "warm" (a live parsed
/// DOM + style + layout tree). The rest are hibernated down to a handful of
/// strings (url, title, scroll position), so a session can hold hundreds of
/// open tabs without hundreds of live engines in memory at once.
pub struct TabManager {
    tabs: Vec<Tab>,
    lru: VecDeque<TabId>, // front = least recently used, back = most recently used
    next_id: TabId,
    max_active: usize,
    viewport: (u32, u32),
}

impl TabManager {
    pub fn new(max_active: usize, viewport: (u32, u32)) -> Self {
        Self {
            tabs: Vec::new(),
            lru: VecDeque::new(),
            next_id: 0,
            max_active,
            viewport,
        }
    }

    pub fn open_tab(&mut self, url: impl Into<String>, html: &str) -> TabId {
        let id = self.next_id;
        self.next_id += 1;
        let url = url.into();
        self.tabs.push(Tab {
            id,
            title: url.clone(),
            url: url.clone(),
            state: TabState::Active,
            history: vec![url],
            history_pos: 0,
            engine: Some(PageEngine::from_html(html, self.viewport)),
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
    /// waking a hibernated tab, reloading, and back/forward navigation.
    fn load(&mut self, id: TabId, url: &str, html: &str) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.url = url.to_string();
            tab.title = url.to_string();
            tab.engine = Some(PageEngine::from_html(html, self.viewport));
            tab.state = TabState::Active;
        }
        self.lru.retain(|&t| t != id);
        self.lru.push_back(id);
        self.enforce_budget();
    }

    /// Wake a hibernated tab (or no-op if already active) and mark it most
    /// recently used, without changing its URL or history.
    pub fn activate(&mut self, id: TabId, html_for_wake: &str) {
        let Some(tab) = self.tabs.iter().find(|t| t.id == id) else {
            return;
        };
        if tab.state == TabState::Hibernated {
            let url = tab.url.clone();
            self.load(id, &url, html_for_wake);
        } else {
            self.lru.retain(|&t| t != id);
            self.lru.push_back(id);
        }
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

    fn enforce_budget(&mut self) {
        let active_count = self.tabs.iter().filter(|t| t.state == TabState::Active).count();
        if active_count <= self.max_active {
            return;
        }
        let mut to_hibernate = active_count - self.max_active;
        for id in self.lru.iter() {
            if to_hibernate == 0 {
                break;
            }
            if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == *id) {
                if tab.state == TabState::Active {
                    tab.engine = None;
                    tab.state = TabState::Hibernated;
                    to_hibernate -= 1;
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

    pub fn hibernated_count(&self) -> usize {
        self.tabs.iter().filter(|t| t.state == TabState::Hibernated).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEMO: &str = "<html><body><p>demo</p></body></html>";

    #[test]
    fn opening_tabs_beyond_budget_hibernates_the_oldest() {
        let mut mgr = TabManager::new(2, (800, 600));
        let a = mgr.open_tab("a", DEMO);
        let b = mgr.open_tab("b", DEMO);
        let c = mgr.open_tab("c", DEMO);

        assert_eq!(mgr.active_count(), 2);
        assert_eq!(mgr.hibernated_count(), 1);
        assert_eq!(mgr.tab(a).unwrap().state, TabState::Hibernated);
        assert_eq!(mgr.tab(b).unwrap().state, TabState::Active);
        assert_eq!(mgr.tab(c).unwrap().state, TabState::Active);
    }

    #[test]
    fn activating_a_hibernated_tab_wakes_it_and_hibernates_the_lru_tab() {
        let mut mgr = TabManager::new(2, (800, 600));
        let a = mgr.open_tab("a", DEMO);
        let _b = mgr.open_tab("b", DEMO);
        let _c = mgr.open_tab("c", DEMO); // hibernates a

        mgr.activate(a, DEMO);

        assert_eq!(mgr.tab(a).unwrap().state, TabState::Active);
        assert_eq!(mgr.active_count(), 2);
        assert_eq!(mgr.hibernated_count(), 1);
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
    fn hundreds_of_tabs_stay_within_the_active_budget() {
        let mut mgr = TabManager::new(8, (800, 600));
        for i in 0..300 {
            mgr.open_tab(format!("tab-{i}"), DEMO);
        }
        assert_eq!(mgr.tabs().len(), 300);
        assert_eq!(mgr.active_count(), 8);
        assert_eq!(mgr.hibernated_count(), 292);
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
}
