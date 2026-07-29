//! Smoke test for gosub-engine's own from-scratch HTML5 parser
//! (<https://github.com/gosub-io/gosub-engine>, MIT), the most actively
//! maintained "modular embeddable browser engine" project surfaced by the
//! rusty-web-browsers-any survey in ARCHITECTURE.md.
//!
//! This is not wired into Melora's tab pipeline yet -- it only proves the
//! dependency builds and parses correctly, as groundwork for evaluating
//! gosub-engine as a second engine backend alongside Blitz.

use gosub_css3::system::Css3System;
use gosub_html5::document::builder::DocumentBuilderImpl;
use gosub_html5::document::document_impl::DocumentImpl;
use gosub_html5::parser::Html5Parser;
use gosub_interface::config::ModuleConfiguration;
use gosub_shared::byte_stream::{ByteStream, Encoding};

#[derive(Clone, Debug, PartialEq)]
struct Config;

impl ModuleConfiguration for Config {
    type CssSystem = Css3System;
    type Document = DocumentImpl<Self>;
    type HtmlParser = Html5Parser<'static, Self>;
}

#[test]
fn gosub_html5_parses_and_normalizes_a_fragment() {
    let mut stream = ByteStream::from_str("<p>Hello <b>Melora</b></p>", Encoding::UTF8);
    let mut doc = DocumentBuilderImpl::new_document::<Config>(None);
    let _ = Html5Parser::<Config>::parse_document(&mut stream, &mut doc, None);

    // The HTML5 tree-construction algorithm should have inserted the implied
    // <html>/<head>/<body> around the fragment, per spec.
    let rendered = format!("{doc}");
    assert!(rendered.contains("<html>"));
    assert!(rendered.contains("<body>"));
    assert!(rendered.contains("<b>"));
    assert!(rendered.contains("Melora"));
}
