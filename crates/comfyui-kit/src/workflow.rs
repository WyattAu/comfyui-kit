//! A thin, strict wrapper over a ComfyUI workflow graph (the JSON you
//! POST to `/prompt`).
//!
//! ComfyUI workflows are `{ node_id: { class_type, inputs } }` maps.
//! This wrapper adds guarded mutation — setting an input on a missing
//! node is an error, not a silent insert — plus seed/text conveniences
//! for the two most common programmatic tweaks.

use crate::error::{ComfyError, Result};
use serde_json::Value;

/// A ComfyUI workflow graph.
#[derive(Debug, Clone, PartialEq)]
pub struct Workflow(Value);

impl Workflow {
    /// Wrap an existing workflow JSON object.
    ///
    /// # Errors
    /// Fails when `json` is not a JSON object.
    pub fn from_json(json: Value) -> Result<Workflow> {
        if !json.is_object() {
            return Err(ComfyError::Workflow(
                "workflow must be a JSON object of node_id → node".to_owned(),
            ));
        }
        Ok(Workflow(json))
    }

    /// Parse a workflow from a JSON string.
    ///
    /// # Errors
    /// Fails on invalid JSON or a non-object root.
    pub fn parse(text: &str) -> Result<Workflow> {
        let json: Value = serde_json::from_str(text)?;
        Workflow::from_json(json)
    }

    /// The underlying graph (pass to [`crate::Client::queue_prompt`]).
    #[must_use]
    pub fn as_json(&self) -> &Value {
        &self.0
    }

    /// Node ids present in the graph.
    #[must_use]
    pub fn node_ids(&self) -> Vec<String> {
        match self.0.as_object() {
            Some(map) => map.keys().cloned().collect(),
            None => Vec::new(),
        }
    }

    /// Set `inputs.<field>` on a node.
    ///
    /// # Errors
    /// Fails when the node does not exist or has no `inputs` object.
    pub fn set_input(&mut self, node_id: &str, field: &str, value: Value) -> Result<()> {
        let node = self
            .0
            .get_mut(node_id)
            .and_then(|n| n.as_object_mut())
            .ok_or_else(|| {
                ComfyError::Workflow(format!("node `{node_id}` not found in workflow"))
            })?;
        let inputs = node
            .entry("inputs")
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        let Some(inputs) = inputs.as_object_mut() else {
            return Err(ComfyError::Workflow(format!(
                "node `{node_id}` inputs is not an object"
            )));
        };
        inputs.insert(field.to_owned(), value);
        Ok(())
    }

    /// Set a node's `seed` input (KSampler-family nodes).
    ///
    /// # Errors
    /// Same as [`Workflow::set_input`].
    pub fn set_seed(&mut self, node_id: &str, seed: u64) -> Result<()> {
        self.set_input(node_id, "seed", Value::from(seed))
    }

    /// Set a node's `text` input (prompt/negative text nodes).
    ///
    /// # Errors
    /// Same as [`Workflow::set_input`].
    pub fn set_text(&mut self, node_id: &str, text: &str) -> Result<()> {
        self.set_input(node_id, "text", Value::from(text))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    const GRAPH: &str = r#"{
        "3": { "class_type": "KSampler", "inputs": { "seed": 1, "model": ["4", 0] } },
        "6": { "class_type": "CLIPTextEncode", "inputs": { "text": "a cat" } }
    }"#;

    #[test]
    fn parse_accepts_object_rejects_array() {
        assert!(Workflow::parse(GRAPH).is_ok());
        assert!(Workflow::parse("[]").is_err());
    }

    #[test]
    fn set_input_on_existing_node() {
        let mut wf = Workflow::parse(GRAPH).unwrap();
        wf.set_input("3", "cfg", Value::from(7.5)).unwrap();
        let cfg = wf.0.pointer("/3/inputs/cfg").unwrap();
        assert_eq!(cfg.as_f64(), Some(7.5));
    }

    #[test]
    fn set_input_on_missing_node_errors() {
        let mut wf = Workflow::parse(GRAPH).unwrap();
        let err = wf.set_input("99", "cfg", Value::from(1)).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn seed_and_text_conveniences() {
        let mut wf = Workflow::parse(GRAPH).unwrap();
        wf.set_seed("3", 42).unwrap();
        wf.set_text("6", "a dog").unwrap();
        assert_eq!(wf.0.pointer("/3/inputs/seed").unwrap().as_u64(), Some(42));
        assert_eq!(
            wf.0.pointer("/6/inputs/text").unwrap().as_str(),
            Some("a dog")
        );
        assert_eq!(wf.node_ids().len(), 2);
    }
}
