use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use blitz_net::Provider;
use blitz_traits::net::{Request, SharedCallback, Url};

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

/// Bridges Blitz's async networking (`blitz-net`, which needs a tokio
/// runtime) to Slint's single-threaded UI event loop (which doesn't).
///
/// A dedicated OS thread owns a tokio runtime and does the actual fetching.
/// Only `Send`-safe data (ids, URLs, bytes, `NavIntent`) crosses the thread
/// boundary -- nothing here ever touches `TabManager` or any Slint type
/// directly from the background thread, since neither is `Send`. The
/// receiving end is meant to be drained on the UI thread via a
/// `slint::Timer` (see `main.rs`), where applying `NavIntent` to the
/// `TabManager` is safe.
pub struct Network {
    tx: tokio::sync::mpsc::UnboundedSender<(TabId, Url, NavIntent)>,
}

impl Network {
    pub fn spawn() -> (Self, Receiver<(TabId, NavIntent, FetchOutcome)>) {
        let (fetch_tx, mut fetch_rx) =
            tokio::sync::mpsc::unbounded_channel::<(TabId, Url, NavIntent)>();
        let (result_tx, result_rx) = channel::<(TabId, NavIntent, FetchOutcome)>();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("failed to start network runtime");
            rt.block_on(async move {
                let noop: SharedCallback<()> =
                    Arc::new(|_id: usize, _res: Result<(), Option<String>>| {});
                let provider = Arc::new(Provider::<()>::new(noop));

                while let Some((id, url, intent)) = fetch_rx.recv().await {
                    let provider = provider.clone();
                    let result_tx = result_tx.clone();
                    tokio::spawn(async move {
                        let request = Request::get(url);
                        let outcome = match provider.fetch_async(request).await {
                            Ok((_final_url, bytes)) => FetchOutcome::Ok {
                                bytes: bytes.to_vec(),
                            },
                            Err(e) => FetchOutcome::Err {
                                message: format!("{e:?}"),
                            },
                        };
                        let _ = result_tx.send((id, intent, outcome));
                    });
                }
            });
        });

        (Network { tx: fetch_tx }, result_rx)
    }

    pub fn fetch(&self, id: TabId, url: Url, intent: NavIntent) {
        let _ = self.tx.send((id, url, intent));
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

        let (network, results) = Network::spawn();
        let mut mgr = TabManager::new(8, (800, 600));
        let id = mgr.open_tab("melora://start", "<html></html>");

        network.fetch(id, url.clone(), NavIntent::Navigate(url.to_string()));

        let (got_id, intent, outcome) = results
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
