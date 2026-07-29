mod engine;
mod tabs;

slint::include_modules!();

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use slint::{ModelRc, VecModel};
use tabs::{TabId, TabManager, TabState};

const MAX_ACTIVE_TABS: usize = 8;
const VIEWPORT: (u32, u32) = (1000, 560);

fn placeholder_html(url: &str) -> String {
    format!(
        "<html><body><h1>{url}</h1><p>Melora's parse/style/layout pipeline (html5ever + \
         Stylo + Taffy, assembled via Blitz) ran successfully for this address. Network \
         fetching and on-screen page painting are not wired up yet — see ARCHITECTURE.md.</p>\
         </body></html>"
    )
}

fn refresh(window: &MainWindow, manager: &TabManager, active_id: Option<TabId>) {
    let items: Vec<TabItem> = manager
        .tabs()
        .iter()
        .map(|t| TabItem {
            id: t.id as i32,
            title: t.title.clone().into(),
            url: t.url.clone().into(),
            active: Some(t.id) == active_id,
            hibernated: t.state == TabState::Hibernated,
        })
        .collect();
    window.set_tabs(ModelRc::new(VecModel::from(items)));

    window.set_status_text(
        format!(
            "{} tabs · {} active · {} hibernated",
            manager.tabs().len(),
            manager.active_count(),
            manager.hibernated_count(),
        )
        .into(),
    );

    if let Some(id) = active_id {
        window.set_can_go_back(manager.can_go_back(id));
        window.set_can_go_forward(manager.can_go_forward(id));
        if let Some(tab) = manager.tab(id) {
            window.set_address_text(tab.url.clone().into());
            window.set_page_text(format!("{}\n\n{} DOM nodes after layout.", tab.title, tab.node_count()).into());
        }
    }
}

fn main() {
    let window = MainWindow::new().unwrap();
    let manager = Rc::new(RefCell::new(TabManager::new(MAX_ACTIVE_TABS, VIEWPORT)));
    let active_id = Rc::new(Cell::new(None::<TabId>));

    {
        let mut mgr = manager.borrow_mut();
        let id = mgr.open_tab("melora://start", &placeholder_html("melora://start"));
        active_id.set(Some(id));
    }
    refresh(&window, &manager.borrow(), active_id.get());

    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        window.on_new_tab(move || {
            let window = window_weak.unwrap();
            let mut mgr = manager.borrow_mut();
            let id = mgr.open_tab("melora://new-tab", &placeholder_html("melora://new-tab"));
            active_id.set(Some(id));
            refresh(&window, &mgr, active_id.get());
        });
    }

    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        window.on_close_tab(move |id| {
            let window = window_weak.unwrap();
            let mut mgr = manager.borrow_mut();
            let id = id as TabId;
            mgr.close_tab(id);
            if active_id.get() == Some(id) {
                active_id.set(mgr.tabs().last().map(|t| t.id));
            }
            refresh(&window, &mgr, active_id.get());
        });
    }

    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        window.on_activate_tab(move |id| {
            let window = window_weak.unwrap();
            let mut mgr = manager.borrow_mut();
            let id = id as TabId;
            let url = mgr.tab(id).map(|t| t.url.clone()).unwrap_or_default();
            mgr.activate(id, &placeholder_html(&url));
            active_id.set(Some(id));
            refresh(&window, &mgr, active_id.get());
        });
    }

    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        window.on_navigate(move |text| {
            let window = window_weak.unwrap();
            let mut mgr = manager.borrow_mut();
            if let Some(id) = active_id.get() {
                let url = text.to_string();
                let html = placeholder_html(&url);
                mgr.navigate(id, url, &html);
            }
            refresh(&window, &mgr, active_id.get());
        });
    }

    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        window.on_reload(move || {
            let window = window_weak.unwrap();
            let mut mgr = manager.borrow_mut();
            if let Some(id) = active_id.get() {
                let url = mgr.tab(id).map(|t| t.url.clone()).unwrap_or_default();
                let html = placeholder_html(&url);
                mgr.reload(id, &html);
            }
            refresh(&window, &mgr, active_id.get());
        });
    }

    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        window.on_go_back(move || {
            let window = window_weak.unwrap();
            let mut mgr = manager.borrow_mut();
            if let Some(id) = active_id.get() {
                let url = mgr.tab(id).map(|t| t.url.clone()).unwrap_or_default();
                let html = placeholder_html(&url);
                mgr.go_back(id, &html);
            }
            refresh(&window, &mgr, active_id.get());
        });
    }

    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        window.on_go_forward(move || {
            let window = window_weak.unwrap();
            let mut mgr = manager.borrow_mut();
            if let Some(id) = active_id.get() {
                let url = mgr.tab(id).map(|t| t.url.clone()).unwrap_or_default();
                let html = placeholder_html(&url);
                mgr.go_forward(id, &html);
            }
            refresh(&window, &mgr, active_id.get());
        });
    }

    window.run().unwrap();
}
