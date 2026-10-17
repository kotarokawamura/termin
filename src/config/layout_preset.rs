use serde::{Deserialize, Serialize};

/// Serializable layout preset structure for saving and restoring workspace splits.
#[derive(Clone, Debug, Serialize, Deserialize)]
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
            title: "Terminal 1".to_string(),
            command: None,
        }
    }

}
