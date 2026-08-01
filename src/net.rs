use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use blitz_dom::net::Resource;
use blitz_net::Provider;
use blitz_traits::net::{NetProvider, Request, SharedCallback, Url};

use crate::tabs::{TabId, TabManager};

pub enum FetchOutcome {
    Ok { bytes: Vec<u8> },
    Err { message: String },
}

/// Which `TabManager` mutation a fetch result should trigger once it comes
/// back, and the URL that was (or will be) loaded. Carrying this alongside
/// every in-flight fetch is what lets a single network layer serve
/// navigation, reload, back/forward, and waking a hibernated tab, instead
/// of duplicating the fetch-and-apply plumbing four times.
pub enum NavIntent {
    Navigate(String),
    Reload(String),
    Back(String),
    Forward(String),
    Wake(String),
}

impl NavIntent {
    pub fn target(&self) -> &str {
        match self {
            NavIntent::Navigate(u)
            | NavIntent::Reload(u)
            | NavIntent::Back(u)
            | NavIntent::Forward(u)
            | NavIntent::Wake(u) => u,
        }
    }

    /// Replaces this intent's target string, keeping its variant. Used by
    /// `start_load` to swap the raw, possibly-schemeless text the user
    /// typed (e.g. `"duckduckgo.com"`) for the fully resolved absolute URL
    /// `resolve_typed_url` already computed for the fetch itself -- see its
    /// call site for why this matters: `Navigate`'s string becomes the
    /// fetched document's own base URL (`PageEngine::from_html`'s `url`
    /// parameter), and blitz-dom needs a real absolute URL there to resolve
    /// any relative `<link>`/`<script>` reference on the page at all. Only
    /// `Navigate`'s string is ever actually read downstream (`apply`,
    /// below) -- `Reload`/`Back`/`Forward`/`Wake` look their real target up
    /// from the tab's own history instead -- but resolving one more time is
    /// harmless for them too.
    pub fn with_target(self, resolved: String) -> Self {
        match self {
            NavIntent::Navigate(_) => NavIntent::Navigate(resolved),
            NavIntent::Reload(_) => NavIntent::Reload(resolved),
            NavIntent::Back(_) => NavIntent::Back(resolved),
            NavIntent::Forward(_) => NavIntent::Forward(resolved),
            NavIntent::Wake(_) => NavIntent::Wake(resolved),
        }
    }

    pub fn apply(&self, mgr: &mut TabManager, id: TabId, html: &str) {
        match self {
            NavIntent::Navigate(u) => mgr.navigate(id, u.clone(), html),
            NavIntent::Reload(_) => mgr.reload(id, html),
            NavIntent::Back(_) => mgr.go_back(id, html),
            NavIntent::Forward(_) => mgr.go_forward(id, html),
            // Only reached via TabManager::activate's NeedsRefetch fallback
            // -- the common case wakes from the local compressed snapshot
            // and never touches the network at all.
            NavIntent::Wake(_) => mgr.force_activate(id, html),
        }
    }
}

/// The two event streams a `Network` produces, both meant to be drained on
/// the UI thread (see `main.rs`'s `slint::Timer`).
pub struct NetworkEvents {
    /// Completed top-level page fetches (navigate/reload/back/forward/wake).
    pub pages: Receiver<(TabId, NavIntent, FetchOutcome)>,
    /// Completed sub-resource fetches (stylesheets, images, fonts),
    /// triggered internally by blitz-dom as it parses a page -- tagged with
    /// the document id they belong to, since by the time one arrives the
    /// tab may have navigated away, closed, or been demoted (see
    /// `TabManager::apply_resource`).
    pub resources: Receiver<(usize, Resource)>,
}

/// Bridges Blitz's async networking (`blitz-net`, which needs a tokio
/// runtime) to Slint's single-threaded UI event loop (which doesn't).
///
/// A dedicated OS thread owns a tokio runtime and does the actual fetching.
/// Only `Send`-safe data (ids, URLs, bytes, `NavIntent`, `Resource`) crosses
/// the thread boundary -- nothing here ever touches `TabManager` or any
/// Slint type directly from the background thread, since neither is
/// `Send`. The receiving ends (`NetworkEvents`) are meant to be drained on
/// the UI thread, where applying them to the `TabManager` is safe.
pub struct Network {
    tx: tokio::sync::mpsc::UnboundedSender<(TabId, Url, NavIntent)>,
    resource_provider: Arc<dyn NetProvider<Resource>>,
}

impl Network {
    pub fn spawn() -> (Self, NetworkEvents) {
        let runtime = tokio::runtime::Runtime::new().expect("failed to start network runtime");

        // Provider::new() looks up the "current" tokio runtime via a
        // thread-local, so it must run inside an active `enter()` guard.
        // The resulting Handle is captured by value, so the providers stay
        // valid after the guard is dropped -- only construction needs it.
        let _guard = runtime.enter();

        let page_provider = Arc::new(Provider::<()>::new(Arc::new(|_id: usize, _res: Result<(), Option<String>>| {})));

        let (resource_tx, resource_rx) = channel::<(usize, Resource)>();
        let resource_callback: SharedCallback<Resource> = Arc::new(move |doc_id, result| {
            // A failed sub-resource fetch (missing image, 404'd stylesheet)
            // just means that one resource never shows up -- not a reason
            // to do anything else, the page renders without it.
            if let Ok(resource) = result {
                let _ = resource_tx.send((doc_id, resource));
            }
        });
        let resource_provider: Arc<dyn NetProvider<Resource>> =
            Arc::new(Provider::<Resource>::new(resource_callback));

        drop(_guard);

        let (fetch_tx, mut fetch_rx) =
            tokio::sync::mpsc::unbounded_channel::<(TabId, Url, NavIntent)>();
        let (page_result_tx, page_result_rx) = channel::<(TabId, NavIntent, FetchOutcome)>();

        std::thread::spawn(move || {
            runtime.block_on(async move {
                while let Some((id, url, intent)) = fetch_rx.recv().await {
                    let page_provider = page_provider.clone();
                    let page_result_tx = page_result_tx.clone();
                    tokio::spawn(async move {
                        let request = Request::get(url);
                        let outcome = match page_provider.fetch_async(request).await {
                            Ok((_final_url, bytes)) => FetchOutcome::Ok {
                                bytes: bytes.to_vec(),
                            },
                            Err(e) => FetchOutcome::Err {
                                message: format!("{e:?}"),
                            },
                        };
                        let _ = page_result_tx.send((id, intent, outcome));
                    });
                }
            });
        });

        (
            Network { tx: fetch_tx, resource_provider },
            NetworkEvents { pages: page_result_rx, resources: resource_rx },
        )
    }

    pub fn fetch(&self, id: TabId, url: Url, intent: NavIntent) {
        let _ = self.tx.send((id, url, intent));
    }

    /// Shared across every tab's document, so each one can fetch its own
    /// sub-resources (stylesheets, images, fonts) as it parses. See
    /// `TabManager::set_resource_provider`.
    pub fn resource_provider(&self) -> Arc<dyn NetProvider<Resource>> {
        self.resource_provider.clone()
    }
}

/// Turns whatever the user typed in the address bar into a fetchable URL.
/// Bare domains (`example.com`) get `https://` prepended, matching how
/// every mainstream browser's address bar behaves.
pub fn resolve_typed_url(input: &str) -> Result<Url, String> {
    let input = input.trim();
    if let Ok(url) = Url::parse(input) {
        return Ok(url);
    }
    Url::parse(&format!("https://{input}")).map_err(|e| format!("{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabs::TabManager;
    use std::io::Write;
    use std::time::Duration;

    /// Exercises the real background-thread fetch pipeline end to end
    /// (`Network::spawn` -> blitz-net -> channel hop back to a "UI thread"
    /// stand-in -> `NavIntent::apply` -> `TabManager`), the same path
    /// `main.rs`'s Slint timer drives. Uses a `file://` URL instead of a
    /// real HTTP request so the test is hermetic and doesn't depend on
    /// network access being available in whatever environment runs it.
    #[test]
    fn fetch_result_flows_through_to_a_navigated_tab() {
        let path = write_tempfile_html("<html><body><h1>from disk</h1></body></html>");
        let url = Url::from_file_path(&path).unwrap();

        let (network, events) = Network::spawn();
        let mut mgr = TabManager::new(8, (800, 600));
        let id = mgr.open_tab("melora://start", "<html></html>");

        network.fetch(id, url.clone(), NavIntent::Navigate(url.to_string()));

        let (got_id, intent, outcome) = events
            .pages
            .recv_timeout(Duration::from_secs(10))
            .expect("fetch did not complete in time");
        assert_eq!(got_id, id);

        let html = match outcome {
            FetchOutcome::Ok { bytes } => String::from_utf8(bytes).unwrap(),
            FetchOutcome::Err { message } => panic!("fetch failed: {message}"),
        };
        assert!(html.contains("from disk"));

        intent.apply(&mut mgr, id, &html);
        assert_eq!(mgr.tab(id).unwrap().url, url.to_string());
        assert!(mgr.tab(id).unwrap().node_count() > 0);
    }

    /// Exercises the sub-resource half of `Network` end to end: a page
    /// whose stylesheet is fetched via the shared `resource_provider`
    /// actually gets styled once the resulting `Resource` is drained and
    /// applied -- the same path a real page's CSS/images/fonts go through.
    /// Uses `file://` URLs so the test is hermetic.
    #[test]
    fn resource_provider_delivers_a_fetched_stylesheet_to_the_right_doc() {
        let dir = std::env::temp_dir().join(format!(
            "melora-net-subres-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::File::create(dir.join("style.css"))
            .unwrap()
            .write_all(b"#box { background-color: rgb(9,9,9); }")
            .unwrap();
        let html = "<html><head><link rel=\"stylesheet\" href=\"style.css\"></head>\
                     <body style=\"margin:0;\"><div id=\"box\" style=\"width:20px;height:20px;\">\
                     </div></body></html>";
        let html_path = dir.join("page.html");
        std::fs::File::create(&html_path).unwrap().write_all(html.as_bytes()).unwrap();
        let url = Url::from_file_path(&html_path).unwrap();

        let (network, events) = Network::spawn();
        let mut mgr = TabManager::new(8, (20, 20));
        mgr.set_resource_provider(network.resource_provider());
        let id = mgr.open_tab(url.to_string(), html);

        let before = mgr.tab(id).unwrap().paint().unwrap();
        assert_ne!(&before[..4], &[9, 9, 9, 255], "sanity check: shouldn't be pre-styled");

        let (doc_id, resource) = events
            .resources
            .recv_timeout(Duration::from_secs(10))
            .expect("no resource event received");
        assert!(mgr.apply_resource(doc_id, resource));

        let after = mgr.tab(id).unwrap().paint().unwrap();
        assert_eq!(&after[..4], &[9, 9, 9, 255], "external stylesheet was not applied");

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn write_tempfile_html(html: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "melora-net-test-{}-{}.html",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::File::create(&path)
            .unwrap()
            .write_all(html.as_bytes())
            .unwrap();
        path
    }
}
