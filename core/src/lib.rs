//! mini-core: shared logic for Mini Browser across platforms.
//! Desktop uses CEF (Chromium). Android uses GeckoView (or the system WebView).
//! Everything here is UI-engine-agnostic.

pub mod agent;
pub mod agent_api;
pub mod session;
pub mod stealth;
#[cfg(test)]
mod tests;

pub use agent::{AgentCommand, AgentEvent};
pub use session::{Tab, TabId, TabManager};
