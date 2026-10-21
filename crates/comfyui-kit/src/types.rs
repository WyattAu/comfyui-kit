//! Response and payload types from the ComfyUI HTTP API.

use std::collections::BTreeMap;

/// Acknowledgement for a queued workflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedPrompt {
    /// The id to poll history with.
    pub prompt_id: String,
    /// Queue position number.
    pub number: i64,
}

/// Snapshot of the server's execution queues.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QueueStatus {
    /// Currently running entries (raw payloads).
    pub running: Vec<serde_json::Value>,
    /// Pending entries (raw payloads).
    pub pending: Vec<serde_json::Value>,
}

/// A generated file reference returned by saver nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRef {
    /// File name on the server.
    pub filename: String,
    /// Subfolder under the output directory.
    pub subfolder: String,
    /// Folder type: `output`, `temp`, or `input`.
    pub folder_type: String,
}

/// One node's outputs inside a history entry.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NodeOutput {
    /// Image/file references produced by the node.
    pub images: Vec<ImageRef>,
}

/// A completed (or failed) prompt's history entry.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HistoryEntry {
    /// Node id → outputs.
    pub outputs: BTreeMap<String, NodeOutput>,
    /// `status_str`/`completed` raw status payload, when present.
    pub status: Option<serde_json::Value>,
}

/// Acknowledgement for an uploaded input image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadedImage {
    /// File name on the server.
    pub name: String,
    /// Subfolder under the input directory.
    pub subfolder: String,
    /// Folder type (usually `input`).
    pub folder_type: String,
}
