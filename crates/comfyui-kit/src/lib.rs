//! Typed Rust client for the ComfyUI HTTP API.
//!
//! Queue workflows, read history, fetch generated images, upload
//! inputs, and subscribe to live progress — on the estate's transport
//! layer ([`fetch_kit`] for HTTP).
//!
//! ```no_run
//! use comfyui_kit::{Client, Workflow};
//!
//! # async fn demo() -> comfyui_kit::Result<()> {
//! let client = Client::new("http://127.0.0.1:8188");
//! let mut events = client.subscribe().await?;
//! let queued = client.queue_prompt(&Workflow::parse("{}")?).await?;
//! while let Some(event) = events.recv().await {
//!     if let comfyui_kit::ProgressEvent::Executed { node, .. } = event {
//!         println!("node {node} finished");
//!     }
//! }
//! # Ok(())
//! # }
//! ```

pub mod client;
pub mod error;
pub mod progress;
pub mod types;
pub mod workflow;

pub use client::Client;
pub use error::{ComfyError, Result};
pub use progress::{parse_event, ProgressEvent};
pub use types::{HistoryEntry, ImageRef, NodeOutput, QueueStatus, QueuedPrompt, UploadedImage};
pub use workflow::Workflow;
