//! Typed Rust client for the ComfyUI HTTP API.
//!
//! Queue workflows, read history, fetch generated images, and upload
//! inputs — on the estate's transport layer ([`fetch_kit`]). The
//! WebSocket progress stream is documented via [`Client::ws_url`] but
//! not implemented here yet (ComfyUI needs a WS *client*; `ws-kit` is a
//! server framework — wrong layer by its own doctrine).

pub mod client;
pub mod error;
pub mod types;
pub mod workflow;

pub use client::Client;
pub use error::{ComfyError, Result};
pub use types::{HistoryEntry, ImageRef, NodeOutput, QueueStatus, QueuedPrompt, UploadedImage};
pub use workflow::Workflow;
