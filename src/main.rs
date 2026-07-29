mod engine;
mod net;
mod tabs;

slint::include_modules!();

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use slint::{ModelRc, VecModel};
use tabs::{TabId, TabManager, TabState};

const MAX_ACTIVE_TABS: usize = 8;
const VIEWPORT: (u32, u32) = (1000, 560);

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Pages served locally, without a network round-trip.
fn local_page_html(url: &str) -> Option<String> {
    match url {
        "melora://start" => Some(
            "<html><body><h1>Melora</h1><p>Type an address above and press Go. This start \
             page is generated locally; it is not fetched over the network.</p></body></html>"
                .to_string(),
        ),
        "melora://new-tab" => Some(
            "<html><body><h1>New Tab</h1><p>Type an address above and press Go.</p></body></html>"
                .to_string(),
        ),
        _ => None,
    }
}

fn error_page_html(target: &str, message: &str) -> String {
    format!(
        "<html><body><h1>Couldn't load {}</h1><p>{}</p></body></html>",
        html_escape(target),
        html_escape(message)
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

/// Starts loading `intent`'s target into tab `id`. A `melora://` page is
/// applied immediately (no network needed); anything else is handed to the
/// network layer, and the result is applied later, when the timer loop in
/// `main` drains it.
fn start_load(
    id: TabId,
    intent: net::NavIntent,
    network: &net::Network,
    manager: &Rc<RefCell<TabManager>>,
    window: &MainWindow,
) {
    let target = intent.target().to_string();

    if let Some(html) = local_page_html(&target) {
        let mut mgr = manager.borrow_mut();
        intent.apply(&mut mgr, id, &html);
        drop(mgr);
        refresh(window, &manager.borrow(), Some(id));
        return;
    }

    match net::resolve_typed_url(&target) {
        Ok(url) => {
            window.set_status_text(format!("Loading {target}…").into());
            network.fetch(id, url, intent);
        }
        Err(message) => {
            let mut mgr = manager.borrow_mut();
            intent.apply(&mut mgr, id, &error_page_html(&target, &message));
            drop(mgr);
            refresh(window, &manager.borrow(), Some(id));
        }
    }
}

fn main() {
    let window = MainWindow::new().unwrap();
    let manager = Rc::new(RefCell::new(TabManager::new(MAX_ACTIVE_TABS, VIEWPORT)));
    let active_id = Rc::new(Cell::new(None::<TabId>));
    let (network, net_results) = net::Network::spawn();
    let network = Rc::new(network);

    {
        let mut mgr = manager.borrow_mut();
        let id = mgr.open_tab("melora://start", &local_page_html("melora://start").unwrap());
        active_id.set(Some(id));
    }
    refresh(&window, &manager.borrow(), active_id.get());

    // Drains completed network fetches on the UI thread and applies them --
    // the background network thread never touches TabManager or Slint
    // directly, since neither is `Send`. See net.rs.
    let timer = slint::Timer::default();
    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(50),
            move || {
                let Some(window) = window_weak.upgrade() else {
                    return;
                };
                let mut applied = false;
                while let Ok((id, intent, outcome)) = net_results.try_recv() {
                    let mut mgr = manager.borrow_mut();
                    let html = match outcome {
                        net::FetchOutcome::Ok { bytes } => String::from_utf8_lossy(&bytes).into_owned(),
                        net::FetchOutcome::Err { message } => error_page_html(intent.target(), &message),
                    };
                    intent.apply(&mut mgr, id, &html);
                    applied = true;
                }
                if applied {
                    refresh(&window, &manager.borrow(), active_id.get());
                }
            },
        );
    }

    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        window.on_new_tab(move || {
            let window = window_weak.unwrap();
            let mut mgr = manager.borrow_mut();
            let id = mgr.open_tab("melora://new-tab", &local_page_html("melora://new-tab").unwrap());
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
        let network = network.clone();
        window.on_activate_tab(move |id| {
            let window = window_weak.unwrap();
            let id = id as TabId;
            active_id.set(Some(id));

            let needs_wake = manager
                .borrow()
                .tab(id)
                .map(|t| t.state == TabState::Hibernated)
                .unwrap_or(false);

            if needs_wake {
                let url = manager.borrow().tab(id).map(|t| t.url.clone()).unwrap_or_default();
                start_load(id, net::NavIntent::Wake(url), &network, &manager, &window);
            } else {
                manager.borrow_mut().activate(id, "");
                refresh(&window, &manager.borrow(), active_id.get());
            }
        });
    }

    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        let network = network.clone();
        window.on_navigate(move |text| {
            let window = window_weak.unwrap();
            if let Some(id) = active_id.get() {
                start_load(id, net::NavIntent::Navigate(text.to_string()), &network, &manager, &window);
            }
        });
    }

    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        let network = network.clone();
        window.on_reload(move || {
            let window = window_weak.unwrap();
            if let Some(id) = active_id.get() {
                let url = manager.borrow().tab(id).map(|t| t.url.clone()).unwrap_or_default();
                start_load(id, net::NavIntent::Reload(url), &network, &manager, &window);
            }
        });
    }

    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        let network = network.clone();
        window.on_go_back(move || {
            let window = window_weak.unwrap();
            if let Some(id) = active_id.get() {
                if let Some(target) = manager.borrow().peek_back_url(id) {
                    start_load(id, net::NavIntent::Back(target), &network, &manager, &window);
                }
            }
        });
    }

    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        let network = network.clone();
        window.on_go_forward(move || {
            let window = window_weak.unwrap();
            if let Some(id) = active_id.get() {
                if let Some(target) = manager.borrow().peek_forward_url(id) {
                    start_load(id, net::NavIntent::Forward(target), &network, &manager, &window);
                }
            }
        });
    }

    window.run().unwrap();
}
