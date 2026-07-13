mod chat;
mod default;
mod repl;

pub use chat::{ChatHandler, show_chat};
pub use default::DefaultHandler;
pub use repl::ReplHandler;

/// Model-invocation options shared by every handler, bundled to keep
/// `handle()` signatures from accumulating unrelated positional args.
#[derive(Clone)]
pub struct CompletionParams {
    pub model: String,
    pub temperature: f64,
    pub top_p: f64,
    pub stream: bool,
}
