use blitz_dom::{DocumentMutator, LocalName, QualName, ns};
use blitz_html::HtmlDocument;
use boa_engine::object::builtins::JsFunction;
use boa_engine::object::{FunctionObjectBuilder, ObjectInitializer};
use boa_engine::property::Attribute;
use boa_engine::{Context, JsValue, NativeFunction, Source, js_string};
use boa_gc::{Finalize, Trace, empty_trace};
use std::cell::RefCell;
use std::rc::Rc;

/// Wraps plain Rust host state so it can be captured by a Boa native
/// function closure. Contains no JS/GC values (just strings and a raw
/// pointer, see `document`'s doc comment), so it's sound to opt it out of
/// GC tracing.
#[derive(Clone)]
struct HostState(Rc<RefCell<HostStateInner>>);

#[derive(Default)]
struct HostStateInner {
    console: Vec<String>,
    title: Option<String>,
    /// The document currently being scripted against, if any -- set by
    /// `run_with_document` for the duration of a single call and cleared
    /// immediately after, even on panic (see that method's safety
    /// comment). `None` outside such a call, which makes every DOM native
    /// function below a quiet no-op rather than something that needs its
    /// own "is a document mounted" error path -- consistent with how a
    /// missing element/attribute is already handled everywhere else here.
    document: Option<*mut HtmlDocument>,
}

impl Finalize for HostState {}
unsafe impl Trace for HostState {
    empty_trace!();
}

/// Runs a page's inline `<script>` content in a per-page JS context, with
/// a small set of real host bindings: `console.log/warn/error`, a
/// `document.title` setter, and -- while a document is mounted via
/// `run_with_document` -- a narrow but real DOM: `document.getElementById`/
/// `querySelector`, and `.textContent`/`.getAttribute`/`.setAttribute`/
/// `.classList` on the elements they return, all backed by the actual
/// `blitz_dom` tree rather than a shadow copy, so a script that mutates an
/// element is visible in the next paint. Still not a full DOM -- no event
/// listeners, no `createElement`/tree mutation beyond `textContent`, no
/// live `NodeList`s. See ARCHITECTURE.md for what that does and doesn't
/// cover, and why.
pub struct JsEngine {
    context: Context,
    state: HostState,
}

impl JsEngine {
    pub fn new() -> Self {
        let mut context = Context::default();
        // Bounds runaway scripts (e.g. `while (true) {}`). Script
        // execution is synchronous on the UI thread, so without this one
        // bad page's script would hang the whole browser, not just itself.
        context.runtime_limits_mut().set_loop_iteration_limit(1_000_000);

        let state = HostState(Rc::new(RefCell::new(HostStateInner::default())));
        install_console(&mut context, &state);
        install_document(&mut context, &state);
        install_dom_natives(&mut context, &state);

        let mut engine = Self { context, state };
        // JS-level glue for the DOM natives above -- see their doc
        // comments for why this is written in JS rather than built up
        // via `ObjectInitializer`: it's a thin, unremarkable wrapper
        // (id in, `Element` instance out) that's far more readable as
        // actual JS than as native-function plumbing, and it runs once
        // per engine, not per page script.
        engine.run(DOM_PRELUDE);
        engine
    }

    /// Runs `source`. Errors (syntax errors, thrown exceptions, hitting
    /// the loop-iteration limit) are captured into the console log rather
    /// than propagated -- a broken script shouldn't take down the page any
    /// more than a missing image does.
    pub fn run(&mut self, source: &str) {
        if let Err(e) = self.context.eval(Source::from_bytes(source)) {
            self.state.0.borrow_mut().console.push(format!("[uncaught] {e}"));
        }
    }

    /// Runs `source` with `document` mounted for the DOM natives installed
    /// at construction (`getElementById`, `textContent`, etc.) to operate
    /// on.
    ///
    /// SAFETY/lifetime note: `document`'s address is stashed as a raw
    /// pointer in `HostStateInner` for native functions to dereference,
    /// cleared by a drop guard immediately after `run` returns -- even if
    /// `run`'s internal `catch_unwind`-free `eval` call panics partway
    /// through, since the guard's `Drop` still runs during unwind. This is
    /// sound because script execution is fully synchronous (Boa never
    /// re-enters after `context.eval` returns, and nothing here stores a
    /// native function value anywhere that could call back in later), so
    /// nothing can dereference the pointer outside this call's dynamic
    /// extent, where `document` is guaranteed to be the one live, uniquely
    /// borrowed value it points to.
    pub fn run_with_document(&mut self, source: &str, document: &mut HtmlDocument) {
        struct ClearOnDrop(HostState);
        impl Drop for ClearOnDrop {
            fn drop(&mut self) {
                self.0.0.borrow_mut().document = None;
            }
        }

        self.state.0.borrow_mut().document = Some(document as *mut HtmlDocument);
        // Cloning just bumps the `Rc`'s refcount -- cheap, and sidesteps
        // holding a borrow of `self` across the `self.run(source)` call
        // below.
        let _guard = ClearOnDrop(self.state.clone());
        self.run(source);
    }

    /// Drains everything logged via `console.*` since the last call.
    pub fn take_console(&mut self) -> Vec<String> {
        std::mem::take(&mut self.state.0.borrow_mut().console)
    }

    /// The page title, if a script set `document.title`.
    pub fn title(&self) -> Option<String> {
        self.state.0.borrow().title.clone()
    }
}

/// Runs `f` against the currently-mounted document, if any (see
/// `run_with_document`). `None` if no document is mounted, or if the
/// caller passed a node id `f` doesn't find -- callers fold both into
/// whatever "not found" value that particular binding already returns to
/// JS (`undefined`/`null`), rather than distinguishing them.
fn with_document<R>(state: &HostState, f: impl FnOnce(&mut HtmlDocument) -> R) -> Option<R> {
    let ptr = state.0.borrow().document;
    // SAFETY: see `JsEngine::run_with_document`.
    ptr.map(|ptr| f(unsafe { &mut *ptr }))
}

fn js_arg_string(args: &[JsValue], index: usize, ctx: &mut Context) -> boa_engine::JsResult<String> {
    match args.get(index) {
        Some(v) => Ok(v.to_string(ctx)?.to_std_string_escaped()),
        None => Ok(String::new()),
    }
}

fn install_console(context: &mut Context, state: &HostState) {
    fn make(level: &'static str, state: HostState) -> NativeFunction {
        NativeFunction::from_copy_closure_with_captures(
            move |_this, args, state, ctx| {
                // `to_string` runs the JS ToString conversion (same as
                // `String(x)`/template literals) rather than `.display()`,
                // which renders values the way source code would write them
                // -- e.g. a string argument would come out wrapped in
                // quotes, which isn't what `console.log('hi')` should show.
                let mut parts = Vec::with_capacity(args.len());
                for arg in args {
                    parts.push(arg.to_string(ctx)?.to_std_string_escaped());
                }
                state.0.borrow_mut().console.push(format!("[{level}] {}", parts.join(" ")));
                Ok(JsValue::undefined())
            },
            state,
        )
    }

    let console = ObjectInitializer::new(context)
        .function(make("log", state.clone()), js_string!("log"), 0)
        .function(make("warn", state.clone()), js_string!("warn"), 0)
        .function(make("error", state.clone()), js_string!("error"), 0)
        .build();
    context
        .register_global_property(js_string!("console"), console, Attribute::all())
        .expect("console is not already defined on a fresh context");
}

fn install_document(context: &mut Context, state: &HostState) {
    let title_setter: JsFunction = {
        let native = NativeFunction::from_copy_closure_with_captures(
            move |_this, args, state, ctx| {
                let value = match args.first() {
                    Some(v) => v.to_string(ctx)?.to_std_string_escaped(),
                    None => String::new(),
                };
                state.0.borrow_mut().title = Some(value);
                Ok(JsValue::undefined())
            },
            state.clone(),
        );
        FunctionObjectBuilder::new(context.realm(), native)
            .name(js_string!("set title"))
            .length(1)
            .build()
    };

    let document = ObjectInitializer::new(context)
        .accessor(js_string!("title"), None, Some(title_setter), Attribute::all())
        .build();
    context
        .register_global_property(js_string!("document"), document, Attribute::all())
        .expect("document is not already defined on a fresh context");
}

/// Registers the low-level, node-id-based native functions `DOM_PRELUDE`
/// builds `document.getElementById`/`querySelector` and the `Element`
/// class on top of. Kept as plain global functions (not methods on any JS
/// object) since they're purely an implementation detail the prelude
/// wraps -- nothing about their names is part of the surface real page
/// scripts see.
fn install_dom_natives(context: &mut Context, state: &HostState) {
    fn register(context: &mut Context, name: &str, length: usize, native: NativeFunction) {
        let function = FunctionObjectBuilder::new(context.realm(), native)
            .name(js_string!(name))
            .length(length)
            .build();
        context
            .register_global_property(js_string!(name), function, Attribute::all())
            .expect("DOM native names are not already defined on a fresh context");
    }

    register(
        context,
        "__melora_getElementById",
        1,
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, state, ctx| {
                let id = js_arg_string(args, 0, ctx)?;
                // Attribute-selector matching side-steps CSS identifier
                // escaping rules for `#id` selectors (real ids can contain
                // characters that aren't valid unescaped in a `#`-selector)
                // -- only the attribute value's own quoting needs escaping.
                let escaped = id.replace('\\', "\\\\").replace('"', "\\\"");
                let selector = format!("[id=\"{escaped}\"]");
                let found = with_document(state, |doc| doc.query_selector(&selector).ok().flatten())
                    .flatten();
                Ok(found.map(|id| JsValue::from(id as f64)).unwrap_or(JsValue::undefined()))
            },
            state.clone(),
        ),
    );

    register(
        context,
        "__melora_querySelector",
        1,
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, state, ctx| {
                let selector = js_arg_string(args, 0, ctx)?;
                let found = with_document(state, |doc| doc.query_selector(&selector).ok().flatten())
                    .flatten();
                Ok(found.map(|id| JsValue::from(id as f64)).unwrap_or(JsValue::undefined()))
            },
            state.clone(),
        ),
    );

    register(
        context,
        "__melora_getTextContent",
        1,
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, state, ctx| {
                let Some(node_id) = js_arg_node_id(args, 0, ctx)? else {
                    return Ok(JsValue::undefined());
                };
                let text = with_document(state, |doc| doc.get_node(node_id).map(|n| n.text_content()))
                    .flatten();
                Ok(text.map(|t| JsValue::from(js_string!(t.as_str()))).unwrap_or(JsValue::undefined()))
            },
            state.clone(),
        ),
    );

    register(
        context,
        "__melora_setTextContent",
        2,
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, state, ctx| {
                let Some(node_id) = js_arg_node_id(args, 0, ctx)? else {
                    return Ok(JsValue::undefined());
                };
                let value = js_arg_string(args, 1, ctx)?;
                with_document(state, |doc| {
                    let mut mutator = DocumentMutator::new(doc);
                    mutator.remove_and_drop_all_children(node_id);
                    let text_id = mutator.create_text_node(&value);
                    mutator.append_children(node_id, &[text_id]);
                });
                Ok(JsValue::undefined())
            },
            state.clone(),
        ),
    );

    register(
        context,
        "__melora_getAttribute",
        2,
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, state, ctx| {
                let Some(node_id) = js_arg_node_id(args, 0, ctx)? else {
                    return Ok(JsValue::undefined());
                };
                let name = js_arg_string(args, 1, ctx)?;
                let value = with_document(state, |doc| {
                    doc.get_node(node_id)
                        .and_then(|n| n.attr(LocalName::from(name.as_str())))
                        .map(str::to_owned)
                })
                .flatten();
                Ok(value.map(|v| JsValue::from(js_string!(v.as_str()))).unwrap_or(JsValue::undefined()))
            },
            state.clone(),
        ),
    );

    register(
        context,
        "__melora_setAttribute",
        3,
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, state, ctx| {
                let Some(node_id) = js_arg_node_id(args, 0, ctx)? else {
                    return Ok(JsValue::undefined());
                };
                let name = js_arg_string(args, 1, ctx)?;
                let value = js_arg_string(args, 2, ctx)?;
                with_document(state, |doc| {
                    let mut mutator = DocumentMutator::new(doc);
                    let qual_name = QualName::new(None, ns!(), LocalName::from(name));
                    mutator.set_attribute(node_id, qual_name, &value);
                });
                Ok(JsValue::undefined())
            },
            state.clone(),
        ),
    );

    register(
        context,
        "__melora_removeAttribute",
        2,
        NativeFunction::from_copy_closure_with_captures(
            |_this, args, state, ctx| {
                let Some(node_id) = js_arg_node_id(args, 0, ctx)? else {
                    return Ok(JsValue::undefined());
                };
                let name = js_arg_string(args, 1, ctx)?;
                with_document(state, |doc| {
                    let mut mutator = DocumentMutator::new(doc);
                    let qual_name = QualName::new(None, ns!(), LocalName::from(name));
                    mutator.clear_attribute(node_id, qual_name);
                });
                Ok(JsValue::undefined())
            },
            state.clone(),
        ),
    );
}

/// Reads argument `index` as a non-negative, finite number and casts it to
/// a node id. `None` (rather than an error) for anything else, since every
/// caller of these natives only ever passes back a value one of them
/// handed out (see `DOM_PRELUDE`'s `Element` class) -- a malformed id here
/// means "nothing to act on", not a bug to surface to the page.
fn js_arg_node_id(args: &[JsValue], index: usize, ctx: &mut Context) -> boa_engine::JsResult<Option<usize>> {
    let Some(arg) = args.get(index) else {
        return Ok(None);
    };
    let n = arg.to_number(ctx)?;
    Ok(if n.is_finite() && n >= 0.0 { Some(n as usize) } else { None })
}

/// JS-side glue evaluated once per `JsEngine`, before any page script:
/// wraps the low-level `__melora_*` natives into `document.getElementById`/
/// `querySelector` and a small `Element` class carrying `.textContent`,
/// `.getAttribute`/`.setAttribute`/`.removeAttribute`, and `.classList`.
const DOM_PRELUDE: &str = r#"
(function () {
    class Element {
        constructor(nodeId) {
            this._nodeId = nodeId;
        }
        get textContent() {
            const value = __melora_getTextContent(this._nodeId);
            return value === undefined ? '' : value;
        }
        set textContent(value) {
            __melora_setTextContent(this._nodeId, String(value));
        }
        getAttribute(name) {
            const value = __melora_getAttribute(this._nodeId, name);
            return value === undefined ? null : value;
        }
        setAttribute(name, value) {
            __melora_setAttribute(this._nodeId, name, String(value));
        }
        removeAttribute(name) {
            __melora_removeAttribute(this._nodeId, name);
        }
        get classList() {
            const element = this;
            function classes() {
                return (element.getAttribute('class') || '').split(/\s+/).filter(Boolean);
            }
            return {
                contains(name) {
                    return classes().includes(name);
                },
                add(name) {
                    const list = classes();
                    if (!list.includes(name)) {
                        list.push(name);
                        element.setAttribute('class', list.join(' '));
                    }
                },
                remove(name) {
                    element.setAttribute('class', classes().filter((c) => c !== name).join(' '));
                },
            };
        }
    }

    document.getElementById = function (id) {
        const nodeId = __melora_getElementById(id);
        return nodeId === undefined ? null : new Element(nodeId);
    };
    document.querySelector = function (selector) {
        const nodeId = __melora_querySelector(selector);
        return nodeId === undefined ? null : new Element(nodeId);
    };
})();
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_arithmetic_and_captures_console_log() {
        let mut engine = JsEngine::new();
        engine.run("console.log('hello', 1 + 2);");
        assert_eq!(engine.take_console(), vec!["[log] hello 3".to_string()]);
    }

    #[test]
    fn document_title_setter_is_observable_from_rust() {
        let mut engine = JsEngine::new();
        assert_eq!(engine.title(), None);
        engine.run("document.title = 'Changed by script';");
        assert_eq!(engine.title().as_deref(), Some("Changed by script"));
    }

    #[test]
    fn state_persists_across_multiple_run_calls_on_the_same_engine() {
        // Real pages can have several <script> tags sharing one global
        // scope; PageEngine relies on this to run each in document order
        // against the same JsEngine.
        let mut engine = JsEngine::new();
        engine.run("var x = 10;");
        engine.run("console.log(x + 5);");
        assert_eq!(engine.take_console(), vec!["[log] 15".to_string()]);
    }

    #[test]
    fn a_thrown_exception_is_captured_not_propagated() {
        let mut engine = JsEngine::new();
        engine.run("throw new Error('boom');");
        let logs = engine.take_console();
        assert_eq!(logs.len(), 1);
        assert!(logs[0].starts_with("[uncaught]"), "expected an uncaught-error log, got: {logs:?}");
    }

    #[test]
    fn a_runaway_loop_is_bounded_instead_of_hanging() {
        let mut engine = JsEngine::new();
        engine.run("while (true) {}");
        let logs = engine.take_console();
        assert_eq!(logs.len(), 1);
        assert!(logs[0].starts_with("[uncaught]"), "expected a loop-limit error, got: {logs:?}");
    }

    #[test]
    fn dom_natives_without_a_mounted_document_quietly_return_null_rather_than_throwing() {
        let mut engine = JsEngine::new();
        engine.run("console.log(document.getElementById('x'), document.querySelector('#x'));");
        assert_eq!(engine.take_console(), vec!["[log] null null".to_string()]);
    }

    #[test]
    fn get_element_by_id_and_text_content_read_the_real_dom() {
        let mut document =
            HtmlDocument::from_html("<html><body><p id=\"greeting\">hi there</p></body></html>", Default::default());
        let mut engine = JsEngine::new();
        engine.run_with_document(
            "console.log(document.getElementById('greeting').textContent);",
            &mut document,
        );
        assert_eq!(engine.take_console(), vec!["[log] hi there".to_string()]);
    }

    #[test]
    fn set_text_content_mutates_the_real_dom_and_is_visible_after_the_script_runs() {
        let mut document =
            HtmlDocument::from_html("<html><body><p id=\"greeting\">old</p></body></html>", Default::default());
        let mut engine = JsEngine::new();
        engine.run_with_document("document.getElementById('greeting').textContent = 'new';", &mut document);

        let node_id = document.query_selector("#greeting").unwrap().unwrap();
        assert_eq!(document.get_node(node_id).unwrap().text_content(), "new");
    }

    #[test]
    fn get_and_set_attribute_round_trip_through_the_real_dom() {
        let mut document =
            HtmlDocument::from_html("<html><body><div id=\"box\" data-state=\"off\"></div></body></html>", Default::default());
        let mut engine = JsEngine::new();
        engine.run_with_document(
            "const box = document.getElementById('box');\
             console.log(box.getAttribute('data-state'));\
             box.setAttribute('data-state', 'on');\
             console.log(box.getAttribute('data-state'));",
            &mut document,
        );
        assert_eq!(engine.take_console(), vec!["[log] off".to_string(), "[log] on".to_string()]);
    }

    #[test]
    fn class_list_add_remove_and_contains_operate_on_the_class_attribute() {
        let mut document =
            HtmlDocument::from_html("<html><body><div id=\"box\" class=\"a b\"></div></body></html>", Default::default());
        let mut engine = JsEngine::new();
        engine.run_with_document(
            "const box = document.getElementById('box');\
             console.log(box.classList.contains('b'));\
             box.classList.remove('b');\
             box.classList.add('c');\
             console.log(box.classList.contains('b'), box.classList.contains('c'));",
            &mut document,
        );
        assert_eq!(engine.take_console(), vec!["[log] true".to_string(), "[log] false true".to_string()]);

        let node_id = document.query_selector("#box").unwrap().unwrap();
        assert_eq!(document.get_node(node_id).unwrap().attr(LocalName::from("class")), Some("a c"));
    }

    #[test]
    fn document_pointer_is_cleared_after_run_with_document_returns() {
        // A script that stashes an element and calls back into it *after*
        // run_with_document has returned (impossible from a single
        // synchronous script, but worth pinning down directly) must not
        // be able to touch the now-dangling document pointer -- run()
        // outside run_with_document should behave exactly like the "no
        // document mounted" case.
        let mut document = HtmlDocument::from_html("<html><body><p id=\"a\"></p></body></html>", Default::default());
        let mut engine = JsEngine::new();
        engine.run_with_document("void document.getElementById('a');", &mut document);
        engine.run("console.log(document.getElementById('a'));");
        assert_eq!(engine.take_console(), vec!["[log] null".to_string()]);
    }
}
