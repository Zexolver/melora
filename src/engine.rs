use std::sync::Arc;

use anyrender_vello_cpu::VelloCpuImageRenderer;
use blitz_dom::DocumentConfig;
use blitz_dom::local_name;
use blitz_dom::net::Resource;
use blitz_html::HtmlDocument;
use blitz_traits::net::{NetProvider, Url};
use blitz_traits::shell::{ColorScheme, Viewport};

pub struct PageEngine {
    document: HtmlDocument,
    viewport: (u32, u32),
    /// Parsed once from the URL passed to `from_html`, used to resolve
    /// relative `href`s found via `hit_test_href`. `None` if the URL
    /// couldn't be parsed (shouldn't happen in practice -- every caller
    /// gets its URL from `net::resolve_typed_url` or a prior successful
    /// parse -- kept as an `Option` rather than unwrapped so a bad URL
    /// degrades to "links don't resolve" instead of a panic).
    base_url: Option<Url>,
    /// `document.title` as set by an inline `<script>`, if any. `None`
    /// means no script set it (the caller falls back to the URL).
    js_title: Option<String>,
    /// Everything logged via `console.*` by this page's inline scripts,
    /// in order. Not surfaced in the UI yet -- kept for the day there's a
    /// devtools-style panel to show it in.
    console_log: Vec<String>,
}

impl PageEngine {
    /// `url` becomes the document's base URL, used to resolve every
    /// relative reference in the page (stylesheets, images, links).
    /// Without it, blitz-dom panics the moment it hits a relative `href`/
    /// `src` -- which real pages almost always have.
    ///
    /// `net_provider`, when present, is what lets the document fetch its
    /// own sub-resources (stylesheets, images, fonts) as it parses --
    /// without it every page renders with only inline/UA styles. `None` is
    /// fine (and used in this module's tests): blitz-dom falls back to a
    /// provider that simply never resolves those fetches.
    pub fn from_html(
        html: &str,
        url: &str,
        viewport: (u32, u32),
        net_provider: Option<Arc<dyn NetProvider<Resource>>>,
    ) -> Self {
        let mut config = DocumentConfig::default();
        config.viewport = Some(Viewport::new(viewport.0, viewport.1, 1.0, ColorScheme::Light));
        config.base_url = Some(url.to_string());
        config.net_provider = net_provider;
        let mut document = HtmlDocument::from_html(html, config);
        // blitz-dom's layout code is young enough to have real bugs on real
        // pages (see the doc comment on `resolve_layout_safely`) -- caught
        // here rather than letting one bad page take the whole app down.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| document.resolve()));

        // Run inline scripts once, after the initial layout, in document
        // order against one shared JS context (so multiple <script> blocks
        // see each other's globals, like a real browser). A script that
        // panics the JS engine internally shouldn't take the page down any
        // more than a bad stylesheet does, hence the same catch_unwind
        // treatment as layout above.
        let (js_title, console_log) =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_inline_scripts(&document)))
                .unwrap_or_default();

        Self { document, viewport, base_url: Url::parse(url).ok(), js_title, console_log }
    }

    /// `document.title` as last set by an inline script, if any.
    pub fn title_override(&self) -> Option<&str> {
        self.js_title.as_deref()
    }

    /// Everything this page's inline scripts logged via `console.*`.
    pub fn console_log(&self) -> &[String] {
        &self.console_log
    }

    pub fn node_count(&self) -> usize {
        self.document.tree().len()
    }

    /// This document's id, used to route an incoming `Resource` (fetched
    /// asynchronously, arriving later) back to the right tab -- see
    /// `TabManager::apply_resource`.
    pub fn doc_id(&self) -> usize {
        self.document.id()
    }

    /// Applies a fetched sub-resource (stylesheet, image, font) and
    /// re-resolves style/layout to reflect it. Returns `false` if that
    /// re-resolve hit an internal layout panic (contained, not propagated
    /// -- see `resolve_layout_safely`); the page is left showing whatever
    /// it looked like before this resource, rather than crashing.
    pub fn apply_resource(&mut self, resource: Resource) -> bool {
        self.document.load_resource(resource);
        self.resolve_layout_safely()
    }

    /// Rasterizes the current (already-scrolled, already-laid-out) view of
    /// the page to a straight-alpha RGBA8 buffer, `width * height * 4`
    /// bytes, matching `self.viewport`. CPU-only (via `anyrender_vello_cpu`)
    /// -- no GPU surface or window handle needed, which keeps this decoupled
    /// from whatever UI toolkit ends up displaying the result.
    ///
    /// `None` if painting itself panicked internally (contained -- see
    /// `resolve_layout_safely`); callers should fall back to a blank frame
    /// rather than crash.
    pub fn paint(&self) -> Option<Vec<u8>> {
        let (width, height) = self.viewport;
        let document = &self.document;
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            anyrender::render_to_buffer::<VelloCpuImageRenderer, _>(
                |scene| {
                    blitz_paint::paint_scene(scene, document, 1.0, width, height);
                },
                width,
                height,
            )
        }))
        .ok()
    }

    /// Scrolls the page by `(dx, dy)`. Note the sign convention inherited
    /// from `blitz_dom::BaseDocument::scroll_viewport_by`: positive `dy`
    /// moves the *content* down (revealing what's above), so a downward
    /// mouse-wheel scroll -- which should reveal content *below* -- needs a
    /// negative `dy`. Verified in this module's tests.
    pub fn scroll_by(&mut self, dx: f64, dy: f64) {
        self.document.scroll_viewport_by(dx, dy);
    }

    /// Re-lays-out the page for a new viewport size (e.g. the content pane
    /// was resized) and repaints at that size on the next `paint()` call.
    /// Returns `false` if layout panicked internally (contained -- see
    /// `resolve_layout_safely`).
    pub fn resize(&mut self, width: u32, height: u32) -> bool {
        self.viewport = (width, height);
        self.document
            .set_viewport(Viewport::new(width, height, 1.0, ColorScheme::Light));
        self.resolve_layout_safely()
    }

    /// `blitz_dom::BaseDocument::resolve` runs style + layout, and at this
    /// engine's version (0.1.x, young) that includes real bugs reachable
    /// from ordinary real-world markup -- e.g. table layout panicked with
    /// an index-out-of-bounds on pypi.org's real page once its real
    /// stylesheet was applied (a jagged/irregular `<table>`, most likely;
    /// confirmed live, not hypothetical). A renderer bug on one page
    /// shouldn't take the whole browser down, so this is caught here
    /// rather than left to unwind out of every caller. `false` means the
    /// page may be showing stale or incomplete layout; still safe to keep
    /// using (see `paint`, which is independently guarded too).
    fn resolve_layout_safely(&mut self) -> bool {
        let document = &mut self.document;
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| document.resolve())).is_ok()
    }

    /// Hit-tests `(x, y)` (in the document's own pixel space) and, if it
    /// lands on or inside an `<a href>`, resolves that href against the
    /// page's base URL. Returns `None` for anything else (no element hit,
    /// hit an element with no enclosing link, or an unparseable href).
    pub fn hit_test_href(&self, x: f32, y: f32) -> Option<String> {
        let hit = self.document.hit(x, y)?;
        let mut node_id = Some(hit.node_id);
        while let Some(id) = node_id {
            let node = self.document.get_node(id)?;
            if node.data.is_element_with_tag_name(&local_name!("a")) {
                if let Some(href) = node.attr(local_name!("href")) {
                    return self.base_url.as_ref()?.join(href).ok().map(|u| u.to_string());
                }
            }
            node_id = node.parent;
        }
        None
    }
}

/// Depth-first, document-order walk collecting the text content of every
/// inline `<script>` (i.e. one with no `src` -- external scripts aren't
/// fetched, see ARCHITECTURE.md).
fn collect_inline_scripts(document: &HtmlDocument) -> Vec<String> {
    let mut scripts = Vec::new();
    let mut stack = vec![document.root_node().id];
    while let Some(id) = stack.pop() {
        let Some(node) = document.get_node(id) else { continue };
        if node.data.is_element_with_tag_name(&local_name!("script")) && node.attr(local_name!("src")).is_none() {
            scripts.push(node.text_content());
        }
        // Push in reverse so popping the stack visits children in the
        // original left-to-right order.
        stack.extend(node.children.iter().rev());
    }
    scripts
}

/// Runs every inline script found in `document` against one shared
/// `JsEngine`, returning the final `document.title` override (if any
/// script set one) and the combined console log. Skips constructing a JS
/// engine at all for the common case of a script-less page.
fn run_inline_scripts(document: &HtmlDocument) -> (Option<String>, Vec<String>) {
    let scripts = collect_inline_scripts(document);
    if scripts.is_empty() {
        return (None, Vec::new());
    }
    let mut js = crate::js::JsEngine::new();
    for script in &scripts {
        js.run(script);
    }
    (js.title(), js.take_console())
}

#[cfg(test)]
mod tests {
    use super::*;
    use blitz_traits::net::SharedCallback;
    use std::io::Write;
    use std::sync::mpsc::channel;
    use std::time::Duration;

    const BASE: &str = "https://example.test/page";

    #[test]
    fn parses_and_lays_out_a_document() {
        let engine = PageEngine::from_html(
            "<html><body><div style=\"width:300px;height:150px;\">hi</div></body></html>",
            BASE,
            (800, 600),
            None,
        );
        assert!(engine.node_count() > 0);
    }

    /// Regression test: without a base URL, resolving a page's relative
    /// links panics inside blitz-dom the moment it hits one -- which is
    /// exactly what broke loading real pages (e.g. pypi.org, which links
    /// its stylesheet as `/static/css/...`) before `from_html` started
    /// threading the fetched URL through as the document's base.
    #[test]
    fn resolves_relative_hrefs_against_the_page_url_instead_of_panicking() {
        let engine = PageEngine::from_html(
            "<html><head><link rel=\"stylesheet\" href=\"/static/style.css\"></head>\
             <body><img src=\"/img/logo.png\">hi</body></html>",
            BASE,
            (200, 100),
            None,
        );
        assert!(engine.node_count() > 0);
    }

    #[test]
    fn paints_a_non_empty_pixel_buffer() {
        let engine = PageEngine::from_html(
            "<html><body style=\"margin:0;\"><div style=\"width:40px;height:40px;\
             background-color:rgb(200,50,10);\"></div></body></html>",
            BASE,
            (40, 40),
            None,
        );
        let buffer = engine.paint().unwrap();
        assert_eq!(buffer.len(), 40 * 40 * 4);
        // Center pixel should be the div's background color, straight alpha.
        let idx = (20 * 40 + 20) * 4;
        assert_eq!(&buffer[idx..idx + 4], &[200, 50, 10, 255]);
    }

    #[test]
    fn scrolling_down_changes_what_is_painted() {
        let mut html = String::from("<html><body style=\"margin:0;\">");
        for i in 0..30 {
            let color = if i % 2 == 0 { "rgb(255,0,0)" } else { "rgb(0,0,255)" };
            html.push_str(&format!(
                "<div style=\"width:200px;height:40px;background-color:{color};\"></div>"
            ));
        }
        html.push_str("</body></html>");

        let mut engine = PageEngine::from_html(&html, BASE, (200, 100), None);
        let before = engine.paint().unwrap();

        // Negative dy scrolls down (reveals content below) -- see doc comment.
        engine.scroll_by(0.0, -300.0);
        let after = engine.paint().unwrap();

        let differing = before.iter().zip(after.iter()).filter(|(a, b)| a != b).count();
        assert!(
            differing > 1000,
            "expected scrolling to noticeably change the rendered page, only {differing} bytes differed"
        );
    }

    #[test]
    fn resizing_changes_the_painted_buffer_size_and_relayouts() {
        let mut engine = PageEngine::from_html(
            "<html><body style=\"margin:0;\"><div style=\"width:100%;height:20px;\
             background-color:rgb(1,2,3);\"></div></body></html>",
            BASE,
            (100, 50),
            None,
        );
        assert_eq!(engine.paint().unwrap().len(), 100 * 50 * 4);

        assert!(engine.resize(60, 80));
        let buffer = engine.paint().unwrap();
        assert_eq!(buffer.len(), 60 * 80 * 4);
        // The div is width:100%, so at the new (narrower) size its right
        // edge should have moved -- check a pixel just past the old right
        // edge (x=100) is now background (nothing painted there, buffer
        // doesn't even have an x=100 column at width 60) and a pixel near
        // the new right edge is still the div's color.
        let idx = (5 * 60 + 55) * 4;
        assert_eq!(&buffer[idx..idx + 4], &[1, 2, 3, 255]);
    }

    #[test]
    fn hit_test_href_resolves_a_relative_link_against_the_page_url() {
        let engine = PageEngine::from_html(
            "<html><body style=\"margin:0;\"><a href=\"/other-page\" \
             style=\"display:block;width:100px;height:20px;\">click me</a></body></html>",
            BASE,
            (200, 100),
            None,
        );
        let href = engine.hit_test_href(10.0, 10.0);
        assert_eq!(href.as_deref(), Some("https://example.test/other-page"));
    }

    #[test]
    fn hit_test_href_is_none_when_nothing_is_hit_or_no_link_encloses_it() {
        let engine = PageEngine::from_html(
            "<html><body style=\"margin:0;\"><div style=\"width:20px;height:20px;\"></div>\
             </body></html>",
            BASE,
            (200, 100),
            None,
        );
        assert_eq!(engine.hit_test_href(10.0, 10.0), None);
    }

    /// Exercises the real sub-resource pipeline end to end: a stylesheet
    /// fetched over the network (a `file://` URL here, so the test is
    /// hermetic) is applied via `apply_resource` and actually changes what
    /// gets painted -- the same path `main.rs` drives for real HTTP(S)
    /// stylesheets, images, and fonts.
    #[test]
    fn apply_resource_applies_a_fetched_stylesheet() {
        let dir = std::env::temp_dir().join(format!(
            "melora-engine-subres-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let css_path = dir.join("style.css");
        std::fs::File::create(&css_path)
            .unwrap()
            .write_all(b"#box { background-color: rgb(9,9,9); }")
            .unwrap();
        let html_path = dir.join("page.html");
        let html = "<html><head><link rel=\"stylesheet\" href=\"style.css\"></head>\
                     <body style=\"margin:0;\"><div id=\"box\" style=\"width:20px;height:20px;\">\
                     </div></body></html>";
        std::fs::File::create(&html_path).unwrap().write_all(html.as_bytes()).unwrap();
        let base_url = Url::from_file_path(&html_path).unwrap();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();
        let (tx, rx) = channel::<(usize, Resource)>();
        let callback: SharedCallback<Resource> = Arc::new(move |_doc_id, result| {
            if let Ok(resource) = result {
                let _ = tx.send((_doc_id, resource));
            }
        });
        let provider: Arc<dyn NetProvider<Resource>> = Arc::new(blitz_net::Provider::new(callback));
        drop(_guard);
        std::thread::spawn(move || {
            rt.block_on(async move {
                tokio::time::sleep(Duration::from_secs(5)).await;
            });
        });

        let mut engine = PageEngine::from_html(&html, base_url.as_str(), (20, 20), Some(provider));

        let before = engine.paint().unwrap();
        assert_ne!(&before[..4], &[9, 9, 9, 255], "sanity check: shouldn't be pre-styled");

        let (_doc_id, resource) = rx.recv_timeout(Duration::from_secs(5)).expect("no resource event received");
        assert!(engine.apply_resource(resource));

        let after = engine.paint().unwrap();
        assert_eq!(&after[..4], &[9, 9, 9, 255], "external stylesheet's background-color was not applied");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
