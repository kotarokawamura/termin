use serde::{Deserialize, Serialize};
use std::env;

/// Serializable layout preset structure for saving and restoring workspace splits.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[warn(dead_code)]
pub enum LayoutNode {
    Pane {
        title: String,
        command: Option<String>,
    },
    HorizontalSplit {
        children: Vec<LayoutNode>,
    },
    VerticalSplit {
        children: Vec<LayoutNode>,
    },
}



impl Default for LayoutNode {
    fn default() -> Self {
        Self::Pane {
            title: env::var("SHELL").unwrap_or_else(|_| "Terminal 1".to_string()).to_string(),
            command: None,
        }
    }

}
