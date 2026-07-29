use blitz_dom::DocumentConfig;
use blitz_html::HtmlDocument;
use blitz_traits::shell::{ColorScheme, Viewport};

pub struct PageEngine {
    document: HtmlDocument,
}

impl PageEngine {
    pub fn from_html(html: &str, viewport: (u32, u32)) -> Self {
        let mut config = DocumentConfig::default();
        config.viewport = Some(Viewport::new(viewport.0, viewport.1, 1.0, ColorScheme::Light));
        let mut document = HtmlDocument::from_html(html, config);
        document.resolve();
        Self { document }
    }

    pub fn node_count(&self) -> usize {
        self.document.tree().len()
    }

    pub fn root_size(&self) -> (f32, f32) {
        let size = self.document.root_element().final_layout.size;
        (size.width, size.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_lays_out_a_document() {
        let engine = PageEngine::from_html(
            "<html><body><div style=\"width:300px;height:150px;\">hi</div></body></html>",
            (800, 600),
        );
        assert!(engine.node_count() > 0);
        let (w, _h) = engine.root_size();
        assert_eq!(w, 800.0);
    }
}
