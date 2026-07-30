use boa_engine::object::builtins::JsFunction;
use boa_engine::object::{FunctionObjectBuilder, ObjectInitializer};
use boa_engine::property::Attribute;
use boa_engine::{Context, JsValue, NativeFunction, Source, js_string};
use boa_gc::{Finalize, Trace, empty_trace};
use std::cell::RefCell;
use std::rc::Rc;

/// Wraps plain Rust host state so it can be captured by a Boa native
/// function closure. Contains no JS/GC values (just strings), so it's
/// sound to opt it out of GC tracing.
#[derive(Clone)]
struct HostState(Rc<RefCell<HostStateInner>>);

#[derive(Default)]
struct HostStateInner {
    console: Vec<String>,
    title: Option<String>,
}

impl Finalize for HostState {}
unsafe impl Trace for HostState {
    empty_trace!();
}

/// Runs a page's inline `<script>` content in a per-page JS context, with
/// a small set of real host bindings: `console.log/warn/error` and a
/// `document.title` setter. This is not a DOM -- scripts can log and set
/// the page title, nothing else. See ARCHITECTURE.md for what that does
/// and doesn't cover, and why.
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

        Self { context, state }
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

    /// Drains everything logged via `console.*` since the last call.
    pub fn take_console(&mut self) -> Vec<String> {
        std::mem::take(&mut self.state.0.borrow_mut().console)
    }

    /// The page title, if a script set `document.title`.
    pub fn title(&self) -> Option<String> {
        self.state.0.borrow().title.clone()
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
}
