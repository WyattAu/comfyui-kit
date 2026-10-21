//! The HTTP client for ComfyUI's REST API.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;

use crate::error::{ComfyError, Result};
use crate::types::{HistoryEntry, ImageRef, NodeOutput, QueueStatus, QueuedPrompt, UploadedImage};
use crate::workflow::Workflow;

const USER_AGENT: &str = "comfyui-kit/0.1";
const DEFAULT_CLIENT_ID: &str = "comfyui-kit-client";

/// A client for one ComfyUI server's HTTP API.
#[derive(Debug, Clone)]
pub struct Client {
    http: fetch_kit::Client,
    base_url: String,
    client_id: String,
}

/// Body of `POST /prompt`.
#[derive(Debug, Serialize)]
struct PromptBody<'a> {
    prompt: &'a Value,
    client_id: &'a str,
}

/// Query params for `GET /view`.
#[derive(Debug, Serialize)]
struct ViewQuery<'a> {
    filename: &'a str,
    subfolder: &'a str,
    #[serde(rename = "type")]
    folder_type: &'a str,
}

fn nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(1)
}

impl Client {
    /// Create a client for a server base URL, e.g.
    /// `http://127.0.0.1:8188`. A session client id is generated.
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Client {
        Client::new_with_client_id(base_url, format!("{DEFAULT_CLIENT_ID}-{:x}", nanos()))
    }

    /// Create a client with an explicit session client id (used by the
    /// WebSocket progress stream and `queue_prompt`).
    #[must_use]
    pub fn new_with_client_id(base_url: impl Into<String>, client_id: impl Into<String>) -> Client {
        Client {
            http: fetch_kit::Client::builder().user_agent(USER_AGENT).build(),
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            client_id: client_id.into(),
        }
    }

    /// The session client id.
    #[must_use]
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// The WebSocket URL for progress events. v0.2 will subscribe with a
    /// WS client; pass this URL to one of your choosing today.
    #[must_use]
    pub fn ws_url(&self) -> String {
        let ws_base = self.base_url.replacen("http", "ws", 1);
        format!("{ws_base}/ws?clientId={}", self.client_id)
    }

    /// Queue a workflow for execution.
    ///
    /// # Errors
    /// Fails on transport errors, and with [`ComfyError::NodeErrors`]
    /// when the server rejects the workflow (validation payload
    /// preserved).
    pub async fn queue_prompt(&self, workflow: &Workflow) -> Result<QueuedPrompt> {
        let url = format!("{}/prompt", self.base_url);
        let body = PromptBody {
            prompt: workflow.as_json(),
            client_id: &self.client_id,
        };
        match self.http.post(&url).json(&body).send().await {
            Ok(resp) => {
                let value: Value = resp
                    .json()
                    .await
                    .map_err(|e| ComfyError::Fetch(fetch_kit::FetchError::from(e)))?;
                parse_queue_response(value)
            }
            Err(fetch_kit::FetchError::StatusCode { status: 400, body }) => {
                match serde_json::from_str::<Value>(&body) {
                    Ok(value) => parse_queue_response(value),
                    Err(_) => Err(ComfyError::Fetch(fetch_kit::FetchError::StatusCode {
                        status: 400,
                        body,
                    })),
                }
            }
            Err(err) => Err(ComfyError::Fetch(err)),
        }
    }

    /// Fetch the history entry for a prompt id.
    ///
    /// # Errors
    /// Fails on transport errors and [`ComfyError::PromptNotFound`]
    /// when the server has no entry for the id.
    pub async fn history(&self, prompt_id: &str) -> Result<HistoryEntry> {
        let url = format!("{}/history/{prompt_id}", self.base_url);
        let value: Value = self.http.get(&url).json_response().await?;
        parse_history(value, prompt_id)
    }

    /// Snapshot the server's running/pending queues.
    ///
    /// # Errors
    /// Fails on transport errors.
    pub async fn queue(&self) -> Result<QueueStatus> {
        let url = format!("{}/queue", self.base_url);
        let value: Value = self.http.get(&url).json_response().await?;
        Ok(QueueStatus {
            running: value
                .get("queue_running")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default(),
            pending: value
                .get("queue_pending")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default(),
        })
    }

    /// Ask the server to interrupt the currently running prompt.
    ///
    /// # Errors
    /// Fails on transport errors.
    pub async fn interrupt(&self) -> Result<()> {
        let url = format!("{}/interrupt", self.base_url);
        self.http.post(&url).send().await?;
        Ok(())
    }

    /// Download a generated (or input/temp) file from `GET /view`.
    ///
    /// # Errors
    /// Fails on transport errors.
    pub async fn view(&self, image: &ImageRef) -> Result<Vec<u8>> {
        let url = format!("{}/view", self.base_url);
        let params = ViewQuery {
            filename: &image.filename,
            subfolder: &image.subfolder,
            folder_type: &image.folder_type,
        };
        let response = self.http.get(&url).query(&params).send().await?;
        let bytes = response
            .bytes()
            .await
            .map_err(fetch_kit::FetchError::from)?;
        Ok(bytes.to_vec())
    }

    /// Upload bytes as an input image.
    ///
    /// # Errors
    /// Fails on transport errors and non-2xx upload responses.
    pub async fn upload_image(
        &self,
        bytes: &[u8],
        filename: &str,
        overwrite: bool,
    ) -> Result<UploadedImage> {
        let boundary = format!("comfyuikit{:x}", nanos());
        let (content_type, body) = build_multipart(&boundary, "image", filename, bytes, overwrite);
        let url = format!("{}/upload/image", self.base_url);
        let request = self
            .http
            .post(&url)
            .header("content-type", content_type)
            .body(body);
        let response: Value = request.json_response().await?;
        Ok(UploadedImage {
            name: response
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(filename)
                .to_owned(),
            subfolder: response
                .get("subfolder")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_owned(),
            folder_type: response
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("input")
                .to_owned(),
        })
    }
}

/// Build a multipart/form-data body for `POST /upload/image`.
/// Returns `(content-type header value, body bytes)`.
fn build_multipart(
    boundary: &str,
    file_field: &str,
    filename: &str,
    file_bytes: &[u8],
    overwrite: bool,
) -> (String, Vec<u8>) {
    let dash_boundary = format!("--{boundary}\r\n");
    let mut body = Vec::new();
    body.extend_from_slice(dash_boundary.as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"overwrite\"\r\n\r\n{}\r\n",
            if overwrite { "true" } else { "false" }
        )
        .as_bytes(),
    );
    body.extend_from_slice(dash_boundary.as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"{file_field}\"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(file_bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

/// Extract a [`QueuedPrompt`] from `/prompt`'s response — including the
/// 400-shaped body where `node_errors` carries validation failures.
///
/// # Errors
/// Fails when there is no `prompt_id` and no parseable node errors.
pub fn parse_queue_response(value: Value) -> Result<QueuedPrompt> {
    if let Some(id) = value.get("prompt_id").and_then(|v| v.as_str()) {
        let number = value.get("number").and_then(|v| v.as_i64()).unwrap_or(0);
        return Ok(QueuedPrompt {
            prompt_id: id.to_owned(),
            number,
        });
    }
    let node_errors = value.get("node_errors").cloned();
    if node_errors
        .as_ref()
        .is_some_and(|e| e.as_object().is_some_and(|m| !m.is_empty()))
    {
        tracing::warn!("comfyui rejected workflow: node errors present");
        return Err(ComfyError::NodeErrors(node_errors.unwrap_or_default()));
    }
    Err(ComfyError::Other(
        "queue response had neither prompt_id nor node_errors".to_owned(),
    ))
}

/// Extract one prompt's [`HistoryEntry`] from `/history/{id}`.
///
/// # Errors
/// Fails when the payload is not an object or the id is absent.
pub fn parse_history(value: Value, prompt_id: &str) -> Result<HistoryEntry> {
    let entry = value
        .get(prompt_id)
        .ok_or_else(|| ComfyError::PromptNotFound(prompt_id.to_owned()))?;
    let mut outputs = std::collections::BTreeMap::new();
    if let Some(map) = entry.get("outputs").and_then(|v| v.as_object()) {
        for (node_id, out) in map {
            let images = out
                .get("images")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|img| {
                            Some(ImageRef {
                                filename: img.get("filename")?.as_str()?.to_owned(),
                                subfolder: img
                                    .get("subfolder")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_owned(),
                                folder_type: img
                                    .get("type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("output")
                                    .to_owned(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            outputs.insert(node_id.clone(), NodeOutput { images });
        }
    }
    Ok(HistoryEntry {
        outputs,
        status: entry.get("status").cloned(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn parse_queue_accepts_success_shape() {
        let value = serde_json::json!({
            "prompt_id": "abc-123",
            "number": 3,
            "node_errors": {}
        });
        let queued = parse_queue_response(value).unwrap();
        assert_eq!(queued.prompt_id, "abc-123");
        assert_eq!(queued.number, 3);
    }

    #[test]
    fn parse_queue_surfaces_node_errors() {
        let value = serde_json::json!({
            "error": { "type": "prompt_outputs_failed_validation" },
            "node_errors": { "3": { "errors": [ { "message": "bad seed" } ] } }
        });
        let err = parse_queue_response(value).unwrap_err();
        assert!(err.to_string().contains("workflow validation failed"));
    }

    #[test]
    fn parse_queue_rejects_unparseable() {
        let err = parse_queue_response(serde_json::json!({ "unrelated": true })).unwrap_err();
        assert!(err.to_string().contains("neither prompt_id"));
    }

    #[test]
    fn parse_history_maps_outputs() {
        let value = serde_json::json!({
            "abc-123": {
                "outputs": {
                    "9": { "images": [
                        { "filename": "c.png", "subfolder": "sub", "type": "output" }
                    ]}
                },
                "status": { "status_str": "success", "completed": true }
            }
        });
        let entry = parse_history(value, "abc-123").unwrap();
        assert_eq!(entry.outputs.len(), 1);
        let images = &entry.outputs["9"].images;
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].filename, "c.png");
        assert_eq!(images[0].folder_type, "output");
        assert!(entry.status.is_some());
    }

    #[test]
    fn parse_history_missing_id_is_prompt_not_found() {
        let err = parse_history(serde_json::json!({}), "nope").unwrap_err();
        assert!(matches!(err, ComfyError::PromptNotFound(id) if id == "nope"));
    }

    #[test]
    fn multipart_body_structure() {
        let (content_type, body) =
            build_multipart("BOUNDARY", "image", "input.png", b"PNGDATA", true);
        assert!(content_type.starts_with("multipart/form-data; boundary=BOUNDARY"));
        let text = String::from_utf8(body).unwrap();
        assert!(text.contains("--BOUNDARY\r\n"));
        assert!(text.contains("name=\"overwrite\"\r\n\r\ntrue\r\n"));
        assert!(text.contains("filename=\"input.png\""));
        assert!(text.contains("PNGDATA"));
        assert!(text.ends_with("--BOUNDARY--\r\n"));
    }

    #[test]
    fn ws_url_swaps_scheme_and_appends_client_id() {
        let client = Client::new_with_client_id("http://127.0.0.1:8188/", "sess-1");
        assert_eq!(client.ws_url(), "ws://127.0.0.1:8188/ws?clientId=sess-1");
        assert_eq!(client.client_id(), "sess-1");
    }
}
