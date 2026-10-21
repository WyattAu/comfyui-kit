//! Errors surfaced by the ComfyUI client.

/// Fallible result alias used throughout comfyui-kit.
pub type Result<T> = std::result::Result<T, ComfyError>;

/// Errors surfaced by the ComfyUI client.
#[derive(Debug, thiserror::Error)]
pub enum ComfyError {
    /// Transport failure (network, timeout, non-2xx from fetch-kit).
    #[error("fetch error: {0}")]
    Fetch(#[from] fetch_kit::FetchError),

    /// Response body was not the JSON shape we expected.
    #[error("parse error: {0}")]
    Parse(#[from] serde_json::Error),

    /// `POST /prompt` rejected the workflow — the raw `node_errors`
    /// payload is preserved for inspection.
    #[error("workflow validation failed: {0}")]
    NodeErrors(serde_json::Value),

    /// The requested prompt id has no history entry (yet).
    #[error("prompt not found in history: {0}")]
    PromptNotFound(String),

    /// Workflow-level structural problems.
    #[error("{0}")]
    Workflow(String),

    /// Anything else.
    #[error("{0}")]
    Other(String),
}
