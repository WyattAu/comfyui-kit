//! WebSocket progress events from a running prompt.
//!
//! ComfyUI pushes JSON text frames (`executing`, `progress`,
//! `executed`, `status`) and binary preview frames over `/ws`. This
//! module parses the JSON frames into typed [`ProgressEvent`]s; binary
//! frames are surfaced as [`ProgressEvent::Binary`] so callers can
//! render live previews if they want to.

use serde_json::Value;

/// One event from ComfyUI's WebSocket stream.
#[derive(Debug, Clone, PartialEq)]
pub enum ProgressEvent {
    /// The server status changed (queue depth etc.).
    Status {
        /// Raw status payload.
        payload: Value,
    },
    /// A prompt began executing.
    ExecutionStart {
        /// Prompt id.
        prompt_id: String,
    },
    /// A node started (or the prompt finished — `node` is `None`).
    Executing {
        /// Prompt id.
        prompt_id: Option<String>,
        /// Node id, `None` marks prompt completion.
        node: Option<String>,
    },
    /// Sampling progress inside a node (KSampler steps).
    Progress {
        /// Steps done.
        value: f64,
        /// Total steps.
        max: f64,
        /// Prompt id.
        prompt_id: Option<String>,
    },
    /// A node finished executing.
    Executed {
        /// Prompt id.
        prompt_id: String,
        /// Node id.
        node: String,
    },
    /// A binary frame (live preview image data).
    Binary,
    /// A typed frame this version doesn't model.
    Other {
        /// Frame type string.
        frame_type: String,
    },
}

/// Parse one text frame into a [`ProgressEvent`]; `None` for frames
/// that are not JSON objects.
#[must_use]
pub fn parse_event(text: &str) -> Option<ProgressEvent> {
    let value: Value = serde_json::from_str(text).ok()?;
    let frame_type = value.get("type")?.as_str()?.to_owned();
    let data = value.get("data").cloned().unwrap_or(Value::Null);
    let as_str = |key: &str| data.get(key).and_then(|v| v.as_str()).map(str::to_owned);

    let event = match frame_type.as_str() {
        "status" => ProgressEvent::Status { payload: data },
        "execution_start" => ProgressEvent::ExecutionStart {
            prompt_id: as_str("prompt_id").unwrap_or_default(),
        },
        "executing" => ProgressEvent::Executing {
            prompt_id: as_str("prompt_id"),
            node: as_str("node"),
        },
        "progress" => ProgressEvent::Progress {
            value: data.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0),
            max: data.get("max").and_then(|v| v.as_f64()).unwrap_or(0.0),
            prompt_id: as_str("prompt_id"),
        },
        "executed" => ProgressEvent::Executed {
            prompt_id: as_str("prompt_id").unwrap_or_default(),
            node: as_str("node").unwrap_or_default(),
        },
        other => ProgressEvent::Other {
            frame_type: other.to_owned(),
        },
    };
    Some(event)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn parses_progress_with_ids() {
        let event = parse_event(
            r#"{"type": "progress", "data": {"value": 3, "max": 20, "prompt_id": "abc"}}"#,
        )
        .unwrap();
        match event {
            ProgressEvent::Progress {
                value,
                max,
                prompt_id,
            } => {
                assert!((value - 3.0).abs() < 1e-9);
                assert!((max - 20.0).abs() < 1e-9);
                assert_eq!(prompt_id.as_deref(), Some("abc"));
            }
            other => panic!("wrong event: {other:?}"),
        }
    }

    #[test]
    fn executing_with_null_node_marks_completion() {
        let event =
            parse_event(r#"{"type": "executing", "data": {"prompt_id": "abc", "node": null}}"#)
                .unwrap();
        match event {
            ProgressEvent::Executing { prompt_id, node } => {
                assert_eq!(prompt_id.as_deref(), Some("abc"));
                assert_eq!(node, None);
            }
            other => panic!("wrong event: {other:?}"),
        }
    }

    #[test]
    fn status_and_unknown_frames() {
        let status =
            parse_event(r#"{"type": "status", "data": {"status": {"exec_info": {}}}}"#).unwrap();
        assert!(matches!(status, ProgressEvent::Status { .. }));

        let other = parse_event(r#"{"type": "novelty", "data": {}}"#).unwrap();
        assert_eq!(
            other,
            ProgressEvent::Other {
                frame_type: "novelty".to_owned()
            }
        );

        assert!(parse_event("not json").is_none());
    }
}
