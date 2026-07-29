use anyrender_vello_cpu::VelloCpuImageRenderer;
use blitz_dom::DocumentConfig;
use blitz_html::HtmlDocument;
use blitz_traits::shell::{ColorScheme, Viewport};

pub struct PageEngine {
    document: HtmlDocument,
    viewport: (u32, u32),
}

impl PageEngine {
    /// `url` becomes the document's base URL, used to resolve every
    /// relative reference in the page (stylesheets, images, links).
    /// Without it, blitz-dom panics the moment it hits a relative `href`/
    /// `src` -- which real pages almost always have.
    pub fn from_html(html: &str, url: &str, viewport: (u32, u32)) -> Self {
        let mut config = DocumentConfig::default();
        config.viewport = Some(Viewport::new(viewport.0, viewport.1, 1.0, ColorScheme::Light));
        config.base_url = Some(url.to_string());
        let mut document = HtmlDocument::from_html(html, config);
        document.resolve();
        Self { document, viewport }
    }

    pub fn node_count(&self) -> usize {
        self.document.tree().len()
    }

    /// Rasterizes the current (already-scrolled, already-laid-out) view of
    /// the page to a straight-alpha RGBA8 buffer, `width * height * 4`
    /// bytes, matching `self.viewport`. CPU-only (via `anyrender_vello_cpu`)
    /// -- no GPU surface or window handle needed, which keeps this decoupled
    /// from whatever UI toolkit ends up displaying the result.
    pub fn paint(&self) -> Vec<u8> {
        let (width, height) = self.viewport;
        anyrender::render_to_buffer::<VelloCpuImageRenderer, _>(
            |scene| {
                blitz_paint::paint_scene(scene, &self.document, 1.0, width, height);
            },
            width,
            height,
        )
    }

    /// Scrolls the page by `(dx, dy)`. Note the sign convention inherited
    /// from `blitz_dom::BaseDocument::scroll_viewport_by`: positive `dy`
    /// moves the *content* down (revealing what's above), so a downward
    /// mouse-wheel scroll -- which should reveal content *below* -- needs a
    /// negative `dy`. Verified in this module's tests.
    pub fn scroll_by(&mut self, dx: f64, dy: f64) {
        self.document.scroll_viewport_by(dx, dy);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "https://example.test/page";

    #[test]
    fn parses_and_lays_out_a_document() {
        let engine = PageEngine::from_html(
            "<html><body><div style=\"width:300px;height:150px;\">hi</div></body></html>",
            BASE,
            (800, 600),
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
        );
        let buffer = engine.paint();
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

        let mut engine = PageEngine::from_html(&html, BASE, (200, 100));
        let before = engine.paint();

        // Negative dy scrolls down (reveals content below) -- see doc comment.
        engine.scroll_by(0.0, -300.0);
        let after = engine.paint();

        let differing = before.iter().zip(after.iter()).filter(|(a, b)| a != b).count();
        assert!(
            differing > 1000,
            "expected scrolling to noticeably change the rendered page, only {differing} bytes differed"
        );
    }
}
