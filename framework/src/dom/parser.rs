use scraper::{Html, Node as ScraperNode};
use std::sync::atomic::{AtomicU64, Ordering};

use super::{Dom, ElementData, Node};

pub fn parse(html: &str) -> Result<Dom, String> {
    let scraper_dom = Html::parse_document(html);
    let mut id_counter = AtomicU64::new(0);
    let root_node = convert_node(scraper_dom.tree.root(), &mut id_counter);
    Ok(Dom { root: root_node })
}

fn convert_node(scraper_node: ego_tree::NodeRef<ScraperNode>, id_counter: &mut AtomicU64) -> Node {
    match scraper_node.value() {
        ScraperNode::Document => {
            // Wrap the document node in an artificial element so the rest of
            // the code can treat the root as an Element. This collects all
            // children of the document (usually the html element) as children
            // of this synthetic root.
            let children = scraper_node
                .children()
                .map(|c| convert_node(c, id_counter))
                .collect();

            Node::Element(ElementData {
                id: id_counter.fetch_add(1, Ordering::Relaxed),
                tag_name: "document".to_string(),
                attributes: std::collections::HashMap::new(),
                children,
            })
        }
        ScraperNode::Element(el) => {
            let attributes = el.attrs().map(|(k, v)| (k.to_string(), v.to_string())).collect();
            let children = scraper_node
                .children()
                .map(|c| convert_node(c, id_counter))
                .collect();

            Node::Element(ElementData {
                id: id_counter.fetch_add(1, Ordering::Relaxed),
                tag_name: el.name().to_string(),
                attributes,
                children,
            })
        }
        ScraperNode::Text(text) => Node::Text(text.text.to_string()),
        ScraperNode::Comment(comment) => Node::Comment(comment.comment.to_string()),
        _ => Node::Comment("unsupported node type".to_string()),
    }
}
