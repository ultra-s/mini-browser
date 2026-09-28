//! Agent embedding (the "fx acp" idea): an agent connects to the browser over
//! the Agent Client Protocol / local HTTP and drives tabs.
//! Desktop transport: the remote-control HTTP server (src/shared/remote.rs).
//! Android transport: the same commands via a local socket / JNI bridge.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum AgentCommand {
    /// Open a URL, optionally in a background tab.
    Open { url: String, background: bool },
    /// Read the visible text of the active tab (injected via JS in the engine).
    ReadActiveTab,
    /// Close a tab by id.
    Close { tab_id: u64 },
    /// Ask the browser to quit.
    Quit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum AgentEvent {
    Opened { tab_id: u64 },
    PageText { tab_id: u64, text: String },
    Closed { tab_id: u64 },
    Error { message: String },
}
