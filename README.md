# comfyui-kit

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![crates.io](https://img.shields.io/crates/v/comfyui-kit.svg)](https://crates.io/crates/comfyui-kit)

Typed Rust client for the [ComfyUI](https://github.com/comfyanonymous/ComfyUI)
HTTP API: queue workflows, read history, download generations, upload
inputs — built on the estate's transport layer ([fetch-kit](https://crates.io/crates/fetch-kit)).

```rust
use comfyui_kit::{Client, Workflow};

let client = Client::new("http://127.0.0.1:8188");

let mut workflow = Workflow::parse(&workflow_json)?;
workflow.set_text("6", "a cabin in the woods, cinematic")?;
workflow.set_seed("3", 42)?;

let queued = client.queue_prompt(&workflow).await?;
let entry = client.history(&queued.prompt_id).await?;

for image in entry.outputs.values().flat_map(|o| &o.images) {
    let bytes = client.view(image).await?;
}
```

## API surface

| Method | Endpoint | Notes |
|---|---|---|
| `queue_prompt` | `POST /prompt` | 400 + `node_errors` surfaces as `ComfyError::NodeErrors` (payload preserved) |
| `history` | `GET /history/{id}` | maps node outputs → [`ImageRef`]s; missing id = `PromptNotFound` |
| `queue` | `GET /queue` | running/pending snapshots |
| `view` | `GET /view` | downloads a generated file as bytes |
| `upload_image` | `POST /upload/image` | hand-rolled multipart (no extra deps) |
| `interrupt` | `POST /interrupt` | stop the running prompt |
| `ws_url` | — | WebSocket URL for progress events (bring a WS client; v0.2 may subscribe) |

## Design notes

- **Strict queue errors**: workflow validation failures surface as
  `ComfyError::NodeErrors` with the raw payload — not a generic 400.
- **Hand-rolled multipart**: the upload form is built byte-by-byte
  (tested), avoiding a reqwest-multipart dependency.
- **No workflow DSL**: `Workflow` is a guarded wrapper over the graph
  JSON (set inputs on existing nodes only). Workflow *authoring* stays
  in the ComfyUI editor where it belongs.

Apache-2.0. Part of the [WyattAu](https://github.com/WyattAu) toolkit estate.
