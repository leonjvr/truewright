//! The injected walker's output shape and its text rendering
//! (page-snapshot spec: "Text rendering for LLM consumption").

use serde::Deserialize;
use std::fmt::Write;

pub const WALKER_JS: &str = include_str!("../assets/walker.js");

#[derive(Debug, Deserialize)]
pub struct SnapshotResult {
    pub url: String,
    pub title: String,
    pub tree: Option<WalkerNode>,
}

#[derive(Debug, Deserialize)]
pub struct WalkerNode {
    #[allow(dead_code)]
    pub tag: String,
    pub role: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub checked: Option<bool>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub hidden: bool,
    #[serde(rename = "ref", default)]
    pub element_ref: Option<String>,
    #[serde(default)]
    pub children: Vec<WalkerNode>,
}

/// Renders a snapshot as indented text: `page: "<title>" url: <url>` header,
/// then one line per node — role, quoted name, value, state flags, `[ref]`.
pub fn render(result: &SnapshotResult) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "page: \"{}\" url: {}", result.title, result.url);
    if let Some(tree) = &result.tree {
        for child in &tree.children {
            render_node(child, 1, &mut out);
        }
    }
    out
}

fn render_node(node: &WalkerNode, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    let _ = write!(out, "{indent}- {}", node.role);

    if let Some(name) = &node.name {
        if !name.is_empty() {
            let _ = write!(out, " \"{name}\"");
        }
    }
    if let Some(value) = &node.value {
        if !value.is_empty() {
            let _ = write!(out, " value=\"{value}\"");
        }
    }
    if node.checked == Some(true) {
        let _ = write!(out, " checked");
    }
    if node.disabled {
        let _ = write!(out, " disabled");
    }
    if node.hidden {
        let _ = write!(out, " hidden");
    }
    if let Some(r) = &node.element_ref {
        let _ = write!(out, " [{r}]");
    }
    let _ = writeln!(out);

    for child in &node.children {
        render_node(child, depth + 1, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_role_name_value_and_ref() {
        let result = SnapshotResult {
            url: "https://example.com".into(),
            title: "Example".into(),
            tree: Some(WalkerNode {
                tag: "body".into(),
                role: "generic".into(),
                name: None,
                value: None,
                checked: None,
                disabled: false,
                hidden: false,
                element_ref: None,
                children: vec![WalkerNode {
                    tag: "input".into(),
                    role: "textbox".into(),
                    name: Some("Card number".into()),
                    value: Some("4242".into()),
                    checked: None,
                    disabled: false,
                    hidden: false,
                    element_ref: Some("e6".into()),
                    children: vec![],
                }],
            }),
        };

        let rendered = render(&result);
        assert!(rendered.contains("page: \"Example\" url: https://example.com"));
        assert!(rendered.contains("  - textbox \"Card number\" value=\"4242\" [e6]"));
    }

    #[test]
    fn renders_empty_tree() {
        let result = SnapshotResult {
            url: "x".into(),
            title: "t".into(),
            tree: None,
        };
        let rendered = render(&result);
        assert_eq!(rendered, "page: \"t\" url: x\n");
    }
}
