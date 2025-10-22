#[cfg(test)]
mod tests {
    use crate::dom::parser;
    use crate::dom::diff;

    #[test]
    fn test_diff_no_changes() {
        let old_html = "<html><body><h1>Hello</h1></body></html>";
        let new_html = "<html><body><h1>Hello</h1></body></html>";

        let old_dom = parser::parse(old_html).unwrap();
        let new_dom = parser::parse(new_html).unwrap();

        let patches = diff::diff(&old_dom, &new_dom);

        assert!(patches.is_empty());
    }

    #[test]
    fn test_diff_attribute_changes() {
        let old_html = "<html><body><h1 id=\"hello\" class=\"foo\">Hello</h1></body></html>";
        let new_html = "<html><body><h1 id=\"hello\" class=\"bar\">Hello</h1></body></html>";

        let old_dom = parser::parse(old_html).unwrap();
        let new_dom = parser::parse(new_html).unwrap();

        let patches = diff::diff(&old_dom, &new_dom);

        assert_eq!(patches.len(), 1);
        match &patches[0] {
            diff::Patch::SetAttribute { node_id: _, name, value } => {
                assert_eq!(name, "class");
                assert_eq!(value, "bar");
            }
            _ => panic!("Incorrect patch type"),
        }
    }

    #[test]
    fn test_diff_append_child() {
        let old_html = "<html><body></body></html>";
        let new_html = "<html><body><h1>Hello</h1></body></html>";

        let old_dom = parser::parse(old_html).unwrap();
        let new_dom = parser::parse(new_html).unwrap();

        let patches = diff::diff(&old_dom, &new_dom);

        assert_eq!(patches.len(), 1);
        match &patches[0] {
            diff::Patch::AppendChild { parent_id: _, child: _ } => {
                // Correct patch type
            }
            _ => panic!("Incorrect patch type"),
        }
    }

    #[test]
    fn test_diff_remove_attribute() {
        let old_html = "<html><body><h1 class=\"foo\">Hello</h1></body></html>";
        let new_html = "<html><body><h1>Hello</h1></body></html>";

        let old_dom = parser::parse(old_html).unwrap();
        let new_dom = parser::parse(new_html).unwrap();

        let patches = diff::diff(&old_dom, &new_dom);

        assert!(patches.iter().any(|p| matches!(p, diff::Patch::RemoveAttribute { .. })));
    }

    #[test]
    fn test_diff_event_attribute_sets_property() {
        let old_html = "<html><body><h1>Hello</h1></body></html>";
        let new_html = "<html><body><h1 onclick=\"doIt()\">Hello</h1></body></html>";

        let old_dom = parser::parse(old_html).unwrap();
        let new_dom = parser::parse(new_html).unwrap();

        let patches = diff::diff(&old_dom, &new_dom);

        assert!(patches.iter().any(|p| match p {
            diff::Patch::SetProperty { name, value, .. } => name == "onclick" && value.as_deref() == Some("doIt()"),
            _ => false,
        }));
    }

    #[test]
    fn test_diff_event_attribute_removed_with_null() {
        let old_html = "<html><body><h1 onclick=\"doIt()\">Hello</h1></body></html>";
        let new_html = "<html><body><h1 onclick=\"null\">Hello</h1></body></html>";

        let old_dom = parser::parse(old_html).unwrap();
        let new_dom = parser::parse(new_html).unwrap();

        let patches = diff::diff(&old_dom, &new_dom);

        assert!(patches.iter().any(|p| match p {
            diff::Patch::SetProperty { name, value, .. } => name == "onclick" && value.is_none(),
            _ => false,
        }));
    }

    #[test]
    fn test_diff_replace_node_on_tag_change() {
        let old_html = "<html><body><div></div></body></html>";
        let new_html = "<html><body><span></span></body></html>";

        let old_dom = parser::parse(old_html).unwrap();
        let new_dom = parser::parse(new_html).unwrap();

        let patches = diff::diff(&old_dom, &new_dom);

        assert!(patches.iter().any(|p| matches!(p, diff::Patch::ReplaceNode { .. })));
    }

    #[test]
    fn test_diff_text_change() {
        let old_html = "<html><body><p>Hello</p></body></html>";
        let new_html = "<html><body><p>Goodbye</p></body></html>";

        let old_dom = parser::parse(old_html).unwrap();
        let new_dom = parser::parse(new_html).unwrap();

        let patches = diff::diff(&old_dom, &new_dom);

        assert!(patches.iter().any(|p| matches!(p, diff::Patch::SetText { .. })), "expected SetText patch");
    }

    #[test]
    fn test_diff_list_append_remove() {
        let old_html = "<html><body><ul><li>A</li><li>B</li></ul></body></html>";
        let new_html = "<html><body><ul><li>A</li><li>B</li><li>C</li></ul></body></html>";

        let old_dom = parser::parse(old_html).unwrap();
        let new_dom = parser::parse(new_html).unwrap();

        let patches = diff::diff(&old_dom, &new_dom);

        assert!(patches.iter().any(|p| matches!(p, diff::Patch::AppendChild { .. })), "expected AppendChild patch for list append");

        // Now test removal
        let old_html2 = "<html><body><ul><li>A</li><li>B</li><li>C</li></ul></body></html>";
        let new_html2 = "<html><body><ul><li>A</li><li>B</li></ul></body></html>";

        let old_dom2 = parser::parse(old_html2).unwrap();
        let new_dom2 = parser::parse(new_html2).unwrap();

        let patches2 = diff::diff(&old_dom2, &new_dom2);
        assert!(patches2.iter().any(|p| matches!(p, diff::Patch::RemoveChild { .. })), "expected RemoveChild patch for list removal");
    }

    #[test]
    fn test_diff_elements_without_html_id() {
        // Ensure elements without an "id" attribute still get patches using
        // generated internal ids.
        let old_html = "<html><body><div class=\"foo\">X</div></body></html>";
        let new_html = "<html><body><div class=\"bar\">X</div></body></html>";

        let old_dom = parser::parse(old_html).unwrap();
        let new_dom = parser::parse(new_html).unwrap();

        let patches = diff::diff(&old_dom, &new_dom);
        assert!(patches.iter().any(|p| matches!(p, diff::Patch::SetAttribute { name, .. } if name == "class")), "expected SetAttribute for class change");
    }

    #[test]
    fn test_diff_remove_node() {
        let old_html = "<html><body><div><p>Hello</p></div><span></span></body></html>";
        let new_html = "<html><body><span></span></body></html>";

        let old_dom = parser::parse(old_html).unwrap();
        let new_dom = parser::parse(new_html).unwrap();

        let patches = diff::diff(&old_dom, &new_dom);

        assert!(patches.iter().any(|p| matches!(p, diff::Patch::RemoveChild { .. })), "expected RemoveChild patch");
    }

    #[test]
    fn test_diff_remove_multiple_nodes() {
        let old_html = "<html><body><div><p>1</p><p>2</p><p>3</p></div></body></html>";
        let new_html = "<html><body><div><p>2</p></div></body></html>";

        let old_dom = parser::parse(old_html).unwrap();
        let new_dom = parser::parse(new_html).unwrap();

        let patches = diff::diff(&old_dom, &new_dom);
        let remove_patches = patches.iter().filter(|p| matches!(p, diff::Patch::RemoveChild { .. })).count();

        assert_eq!(remove_patches, 2, "expected two RemoveChild patches");
    }

    #[test]
    fn test_diff_replace_text_with_element() {
        let old_html = "<html><body><p>Just text</p></body></html>";
        let new_html = "<html><body><p><span>Now an element</span></p></body></html>";

        let old_dom = parser::parse(old_html).unwrap();
        let new_dom = parser::parse(new_html).unwrap();

        let patches = diff::diff(&old_dom, &new_dom);

        assert!(patches.iter().any(|p| matches!(p, diff::Patch::ReplaceNode { .. })), "expected ReplaceNode patch for text->element change");
    }

    #[test]
    fn test_diff_add_and_remove_attributes() {
        let old_html = "<html><body><div id=\"a\" class=\"b\"></div></body></html>";
        let new_html = "<html><body><div class=\"c\" data-x=\"y\"></div></body></html>";

        let old_dom = parser::parse(old_html).unwrap();
        let new_dom = parser::parse(new_html).unwrap();

        let patches = diff::diff(&old_dom, &new_dom);

        assert!(patches.iter().any(|p| matches!(p, diff::Patch::RemoveAttribute { name, .. } if name == "id")), "expected RemoveAttribute for id");
        assert!(patches.iter().any(|p| matches!(p, diff::Patch::SetAttribute { name, value, .. } if name == "class" && value == "c")), "expected SetAttribute for class");
        assert!(patches.iter().any(|p| matches!(p, diff::Patch::SetAttribute { name, value, .. } if name == "data-x" && value == "y")), "expected SetAttribute for data-x");
    }
}
