use std::collections::HashMap;
use super::{Dom, Node};

use serde::Serialize;

/// Represents a patch operation to be applied to the DOM.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Patch {
    SetAttribute { node_id: u64, name: String, value: String },
    RemoveAttribute { node_id: u64, name: String },
    /// Set a property on the DOM node (used for events and certain input properties)
    SetProperty { node_id: u64, name: String, value: Option<String> },
    ReplaceNode { node_id: u64, new_node: Node },
    SetText { node_id: u64, value: String },
    AppendChild { parent_id: u64, child: Node },
    RemoveChild { parent_id: u64, child_id: u64 },
}

/// Compares two DOM trees and returns a list of patches.
pub fn diff(old_dom: &Dom, new_dom: &Dom) -> Vec<Patch> {
    let mut patches = Vec::new();
    // Start diffing at the root; no parent id for the root node.
    diff_nodes(&old_dom.root, &new_dom.root, None, &mut patches);
    patches
}

fn diff_nodes(old_node: &Node, new_node: &Node, parent_id: Option<u64>, patches: &mut Vec<Patch>) {
    match (old_node, new_node) {
        (Node::Element(old_el), Node::Element(new_el)) => {
            if old_el.tag_name != new_el.tag_name {
                patches.push(Patch::ReplaceNode {
                    node_id: old_el.id,
                    new_node: new_node.clone(),
                });
                return;
            }

            update_attributes(old_el, new_el, patches);

            let old_children = &old_el.children;
            let new_children = &new_el.children;
            let min_len = old_children.len().min(new_children.len());

            for i in 0..min_len {
                // pass the current element's id as the parent id for its children
                diff_nodes(&old_children[i], &new_children[i], Some(old_el.id), patches);
            }

            if old_children.len() > new_children.len() {
                for i in min_len..old_children.len() {
                    if let Node::Element(child) = &old_children[i] {
                        patches.push(Patch::RemoveChild { parent_id: old_el.id, child_id: child.id });
                    }
                }
            } else if new_children.len() > old_children.len() {
                for i in min_len..new_children.len() {
                    patches.push(Patch::AppendChild { parent_id: old_el.id, child: new_children[i].clone() });
                }
            }
        }
        (Node::Text(old_text), Node::Text(new_text)) => {
            if old_text != new_text {
                // Use the parent element's id to identify where to set text.
                if let Some(pid) = parent_id {
                    patches.push(Patch::SetText { node_id: pid, value: new_text.clone() });
                }
            }
        }
        _ => {
            // Node types differ, so we replace the old node with the new one.
            let old_node_id = match old_node {
                Node::Element(el) => el.id,
                Node::Text(_) | Node::Comment(_) => {
                    parent_id.expect("Text or Comment node must have a parent to be replaced")
                }
            };
            patches.push(Patch::ReplaceNode {
                node_id: old_node_id,
                new_node: new_node.clone(),
            });
        }
    }
}

fn update_attributes(old_el: &super::ElementData, new_el: &super::ElementData, patches: &mut Vec<Patch>) {
    let old_attrs = &old_el.attributes;
    let new_attrs = &new_el.attributes;

    // List of events from nanomorph.js — attribute-based event names.
    const EVENTS: &[&str] = &[
        "onclick", "ondblclick", "onmousedown", "onmouseup", "onmouseover",
        "onmousemove", "onmouseout", "onmouseenter", "onmouseleave", "ontouchcancel",
        "ontouchend", "ontouchmove", "ontouchstart", "ondragstart", "ondrag",
        "ondragenter", "ondragleave", "ondragover", "ondrop", "ondragend",
        "onkeydown", "onkeypress", "onkeyup", "onunload", "onabort", "onerror",
        "onresize", "onscroll", "onselect", "onchange", "onsubmit", "onreset",
        "onfocus", "onblur", "oninput", "onanimationend", "onanimationiteration",
        "onanimationstart", "oncontextmenu", "onfocusin", "onfocusout",
    ];

    // Helper to check if a name is a known event
    let is_event = |n: &str| EVENTS.contains(&n);

    // Check for new or changed attributes
    for (name, value) in new_attrs {
        if old_attrs.get(name) != Some(value) {
            // nanomorph treats attribute values "null" and "undefined" as removal
            if value == "null" || value == "undefined" {
                // if it's an event name, clear the property instead
                if is_event(name) {
                    patches.push(Patch::SetProperty { node_id: old_el.id, name: name.clone(), value: None });
                } else {
                    patches.push(Patch::RemoveAttribute { node_id: old_el.id, name: name.clone() });
                }
            } else if is_event(name) {
                // For events, prefer property patches so frontend can attach handlers
                patches.push(Patch::SetProperty { node_id: old_el.id, name: name.clone(), value: Some(value.clone()) });
            } else {
                patches.push(Patch::SetAttribute {
                    node_id: old_el.id,
                    name: name.clone(),
                    value: value.clone(),
                });
            }
        }
    }

    // Check for removed attributes
    for name in old_attrs.keys() {
        if !new_attrs.contains_key(name) {
            if is_event(name) {
                patches.push(Patch::SetProperty { node_id: old_el.id, name: name.clone(), value: None });
            } else {
                patches.push(Patch::RemoveAttribute {
                    node_id: old_el.id,
                    name: name.clone(),
                });
            }
        }
    }
}
