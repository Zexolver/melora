mod engine;
mod js;
mod net;
mod session;
mod swap;
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

fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Rasterizes the active tab's page and wraps it as a Slint image. Falls
/// back to a blank white frame if there's no active tab or (shouldn't
/// normally happen) it has no live engine, rather than showing stale
/// pixels from whatever was painted last.
fn render_page_image(manager: &TabManager, active_id: Option<TabId>) -> slint::Image {
    let (width, height) = manager.viewport();
    let pixels = active_id.and_then(|id| manager.tab(id)).and_then(|tab| tab.paint());

    let mut buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(width, height);
    match pixels {
        Some(pixels) => buffer.make_mut_bytes().copy_from_slice(&pixels),
        None => buffer.make_mut_bytes().fill(255),
    }
    slint::Image::from_rgba8(buffer)
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
            compressed: t.state != TabState::Active,
        })
        .collect();
    window.set_tabs(ModelRc::new(VecModel::from(items)));

    window.set_status_text(
        format!(
            "{} tabs · {} active · {} compressed ({}) · {} swapped ({})",
            manager.tabs().len(),
            manager.active_count(),
            manager.compressed_count(),
            format_bytes(manager.total_compressed_bytes()),
            manager.swapped_count(),
            format_bytes(manager.total_swapped_bytes()),
        )
        .into(),
    );

    window.set_page_image(render_page_image(manager, active_id));

    if let Some(id) = active_id {
        window.set_can_go_back(manager.can_go_back(id));
        window.set_can_go_forward(manager.can_go_forward(id));
        if let Some(tab) = manager.tab(id) {
            window.set_address_text(tab.url.clone().into());
        }
    }
}

/// Snapshots every open tab and writes it to disk, so the next launch can
/// offer to restore this session. Best-effort: a failure here (disk full,
/// permissions) is swallowed rather than interrupting browsing -- losing
/// the ability to restore tabs is much less bad than crashing over it.
fn persist_session(session_store: &session::SessionStore, manager: &Rc<RefCell<TabManager>>, active_id: Option<TabId>) {
    let snapshot = manager.borrow_mut().session_snapshot(active_id);
    let _ = session_store.save(&snapshot);
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
    session_store: &session::SessionStore,
) {
    let target = intent.target().to_string();

    if let Some(html) = local_page_html(&target) {
        let mut mgr = manager.borrow_mut();
        intent.apply(&mut mgr, id, &html);
        drop(mgr);
        refresh(window, &manager.borrow(), Some(id));
        persist_session(session_store, manager, Some(id));
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
            persist_session(session_store, manager, Some(id));
        }
    }
}

fn main() {
    let window = MainWindow::new().unwrap();
    let manager = Rc::new(RefCell::new(TabManager::new(MAX_ACTIVE_TABS, VIEWPORT)));
    let active_id = Rc::new(Cell::new(None::<TabId>));
    let (network, net_events) = net::Network::spawn();
    let network = Rc::new(network);
    manager.borrow_mut().set_resource_provider(network.resource_provider());

    let session_store = Rc::new(session::SessionStore::new());
    // A prior run's tabs, if any, held here until the user answers the
    // restore prompt below -- not applied to `manager` yet, so opening the
    // window doesn't briefly show them then replace them.
    let pending_session = Rc::new(RefCell::new(session_store.load().unwrap_or_default()));

    if pending_session.borrow().is_empty() {
        let mut mgr = manager.borrow_mut();
        let id = mgr.open_tab("melora://start", &local_page_html("melora://start").unwrap());
        active_id.set(Some(id));
    } else {
        window.set_show_restore_prompt(true);
        window.set_restore_prompt_text(
            format!(
                "{} tab(s) from your last session are still residing. Restore them?",
                pending_session.borrow().len()
            )
            .into(),
        );
    }
    refresh(&window, &manager.borrow(), active_id.get());

    // Drains completed network fetches and sub-resource loads on the UI
    // thread and applies them -- the background network thread never
    // touches TabManager or Slint directly, since neither is `Send`. See
    // net.rs.
    let timer = slint::Timer::default();
    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        let session_store = session_store.clone();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(50),
            move || {
                let Some(window) = window_weak.upgrade() else {
                    return;
                };
                let mut applied = false;
                while let Ok((id, intent, outcome)) = net_events.pages.try_recv() {
                    let mut mgr = manager.borrow_mut();
                    let html = match outcome {
                        net::FetchOutcome::Ok { bytes } => String::from_utf8_lossy(&bytes).into_owned(),
                        net::FetchOutcome::Err { message } => error_page_html(intent.target(), &message),
                    };
                    intent.apply(&mut mgr, id, &html);
                    applied = true;
                }
                while let Ok((doc_id, resource)) = net_events.resources.try_recv() {
                    if manager.borrow_mut().apply_resource(doc_id, resource) {
                        applied = true;
                    }
                }
                if applied {
                    refresh(&window, &manager.borrow(), active_id.get());
                    persist_session(&session_store, &manager, active_id.get());
                }
            },
        );
    }

    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        let session_store = session_store.clone();
        window.on_new_tab(move || {
            let window = window_weak.unwrap();
            let mut mgr = manager.borrow_mut();
            let id = mgr.open_tab("melora://new-tab", &local_page_html("melora://new-tab").unwrap());
            active_id.set(Some(id));
            drop(mgr);
            refresh(&window, &manager.borrow(), active_id.get());
            persist_session(&session_store, &manager, active_id.get());
        });
    }

    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        let session_store = session_store.clone();
        window.on_close_tab(move |id| {
            let window = window_weak.unwrap();
            let mut mgr = manager.borrow_mut();
            let id = id as TabId;
            mgr.close_tab(id);
            if active_id.get() == Some(id) {
                active_id.set(mgr.tabs().last().map(|t| t.id));
            }
            drop(mgr);
            refresh(&window, &manager.borrow(), active_id.get());
            persist_session(&session_store, &manager, active_id.get());
        });
    }

    // Session restore prompt: shown at startup only when a prior run left
    // tabs behind (see `pending_session` above). Restoring reconstructs
    // them in the compressed tier and wakes whichever was on screen when
    // the session was saved; declining just clears the stale session file
    // and starts fresh, same as a first run.
    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        let session_store = session_store.clone();
        let pending_session = pending_session.clone();
        window.on_restore_session(move || {
            let window = window_weak.unwrap();
            let tabs = std::mem::take(&mut *pending_session.borrow_mut());
            let mut mgr = manager.borrow_mut();
            let restored_active = mgr.restore_session(tabs);
            let to_activate = restored_active.or_else(|| mgr.tabs().first().map(|t| t.id));
            if let Some(id) = to_activate {
                mgr.activate(id);
            }
            active_id.set(to_activate);
            drop(mgr);
            window.set_show_restore_prompt(false);
            refresh(&window, &manager.borrow(), active_id.get());
            persist_session(&session_store, &manager, active_id.get());
        });
    }

    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        let session_store = session_store.clone();
        let pending_session = pending_session.clone();
        window.on_discard_session(move || {
            let window = window_weak.unwrap();
            pending_session.borrow_mut().clear();
            let _ = session_store.clear();
            let mut mgr = manager.borrow_mut();
            let id = mgr.open_tab("melora://start", &local_page_html("melora://start").unwrap());
            active_id.set(Some(id));
            drop(mgr);
            window.set_show_restore_prompt(false);
            refresh(&window, &manager.borrow(), active_id.get());
        });
    }

    // Hibernate: an explicit, discoverable "close and preserve everything"
    // action -- deliberately not called "Quit", since nothing is discarded.
    // Every tab is already continuously persisted (see `persist_session`
    // call sites throughout this function), so this final save is a
    // belt-and-suspenders flush for the case where the very last thing
    // that happened was opening the window with nothing else done yet;
    // the save itself reads each tab from whichever tier it's actually
    // in -- RAM-compressed or disk-swapped -- so restoring it next launch
    // never touches the network. See `on_close_requested` below for why
    // the OS window-close button gets the same guarantee without needing
    // this button specifically.
    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        let session_store = session_store.clone();
        window.on_hibernate(move || {
            let window = window_weak.unwrap();
            persist_session(&session_store, &manager, active_id.get());
            let _ = window.window().hide();
        });
    }

    // Closing the window any other way (the OS/window-manager close
    // button, Alt+F4, a session logout) should preserve tabs exactly like
    // clicking Hibernate does -- there's no reason "the deliberate button"
    // and "however else the user happens to close it" should behave
    // differently. `HideWindow` (the default response) lets the close
    // proceed as normal after the save.
    {
        let manager = manager.clone();
        let active_id = active_id.clone();
        let session_store = session_store.clone();
        window.window().on_close_requested(move || {
            persist_session(&session_store, &manager, active_id.get());
            slint::CloseRequestResponse::HideWindow
        });
    }

    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        let network = network.clone();
        let session_store = session_store.clone();
        window.on_activate_tab(move |id| {
            let window = window_weak.unwrap();
            let id = id as TabId;
            active_id.set(Some(id));

            // Tries the local compressed/swapped snapshot first -- no
            // network involved in the common case. Only falls back to a
            // real fetch if a tab somehow has no snapshot to wake from.
            //
            // Bound to `result` first, rather than matched on directly:
            // `match manager.borrow_mut().activate(id) { ... }` keeps the
            // `RefMut` temporary alive for the whole match (a well-known
            // Rust footgun), which then panics ("already mutably
            // borrowed") the moment an arm below tries `manager.borrow()`
            // -- confirmed live, not hypothetical: this only ever
            // triggers once a tab is actually woken from the compressed
            // or swapped tier via a real click, which no earlier test or
            // live run happened to exercise.
            let result = manager.borrow_mut().activate(id);
            match result {
                tabs::WakeResult::AlreadyActive
                | tabs::WakeResult::WokeFromCompressed
                | tabs::WakeResult::WokeFromSwap => {
                    refresh(&window, &manager.borrow(), active_id.get());
                    persist_session(&session_store, &manager, active_id.get());
                }
                tabs::WakeResult::NeedsRefetch => {
                    let url = manager.borrow().tab(id).map(|t| t.url.clone()).unwrap_or_default();
                    start_load(id, net::NavIntent::Wake(url), &network, &manager, &window, &session_store);
                }
            }
        });
    }

    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        let network = network.clone();
        let session_store = session_store.clone();
        window.on_navigate(move |text| {
            let window = window_weak.unwrap();
            if let Some(id) = active_id.get() {
                start_load(id, net::NavIntent::Navigate(text.to_string()), &network, &manager, &window, &session_store);
            }
        });
    }

    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        let network = network.clone();
        let session_store = session_store.clone();
        window.on_reload(move || {
            let window = window_weak.unwrap();
            if let Some(id) = active_id.get() {
                let url = manager.borrow().tab(id).map(|t| t.url.clone()).unwrap_or_default();
                start_load(id, net::NavIntent::Reload(url), &network, &manager, &window, &session_store);
            }
        });
    }

    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        let network = network.clone();
        let session_store = session_store.clone();
        window.on_go_back(move || {
            let window = window_weak.unwrap();
            if let Some(id) = active_id.get() {
                if let Some(target) = manager.borrow().peek_back_url(id) {
                    start_load(id, net::NavIntent::Back(target), &network, &manager, &window, &session_store);
                }
            }
        });
    }

    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        let network = network.clone();
        let session_store = session_store.clone();
        window.on_go_forward(move || {
            let window = window_weak.unwrap();
            if let Some(id) = active_id.get() {
                if let Some(target) = manager.borrow().peek_forward_url(id) {
                    start_load(id, net::NavIntent::Forward(target), &network, &manager, &window, &session_store);
                }
            }
        });
    }

    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        window.on_scroll(move |dx, dy| {
            let window = window_weak.unwrap();
            if let Some(id) = active_id.get() {
                manager.borrow_mut().scroll_active(id, dx as f64, dy as f64);
                window.set_page_image(render_page_image(&manager.borrow(), Some(id)));
            }
        });
    }

    // Click-to-navigate: hit-test at the click position (scaled from the
    // TouchArea's actual on-screen size to the page's raster size, since
    // `image-fit: fill` can stretch them apart) and, if it landed on a
    // link, navigate to it.
    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        let network = network.clone();
        let session_store = session_store.clone();
        window.on_click(move |x, y, area_width, area_height| {
            let window = window_weak.unwrap();
            let Some(id) = active_id.get() else { return };

            let (raster_width, raster_height) = manager.borrow().viewport();
            let scale_x = if area_width > 0.0 { raster_width as f32 / area_width } else { 1.0 };
            let scale_y = if area_height > 0.0 { raster_height as f32 / area_height } else { 1.0 };

            let href = manager.borrow().tab(id).and_then(|t| t.hit_test_href(x * scale_x, y * scale_y));
            if let Some(href) = href {
                start_load(id, net::NavIntent::Navigate(href), &network, &manager, &window, &session_store);
            }
        });
    }

    // Re-lays-out the active tab (and every other currently-active one) for
    // the content pane's actual on-screen size whenever it changes -- see
    // the `changed width/height` handlers on the content Rectangle in
    // melora.slint.
    {
        let window_weak = window.as_weak();
        let manager = manager.clone();
        let active_id = active_id.clone();
        window.on_viewport_resized(move |width, height| {
            let Some(window) = window_weak.upgrade() else { return };
            let width = width.max(1.0) as u32;
            let height = height.max(1.0) as u32;
            if manager.borrow().viewport() == (width, height) {
                return;
            }
            manager.borrow_mut().resize_viewport(width, height);
            refresh(&window, &manager.borrow(), active_id.get());
        });
    }

    window.run().unwrap();
}
