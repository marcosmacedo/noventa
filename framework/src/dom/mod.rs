pub mod parser;
pub mod diff;

use std::collections::HashMap;
use serde::Serialize;

/// Represents a node in the DOM tree.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Node {
    /// An element node, containing a tag name, attributes, and children.
    Element(ElementData),
    /// A text node.
    Text(String),
    /// A comment node.
    Comment(String),
}

/// Represents the data associated with an element node.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ElementData {
    pub id: u64, // Unique identifier for this node
    pub tag_name: String,
    pub attributes: HashMap<String, String>,
    pub children: Vec<Node>,
}

impl ElementData {
    /// Helper function to get the HTML id attribute.
    pub fn html_id(&self) -> Option<&String> {
        self.attributes.get("id")
    }
}

#[cfg(test)]
mod diff_test;

/// Represents a parsed HTML document.
#[derive(Debug, Clone, PartialEq)]
pub struct Dom {
    pub root: Node,
}
