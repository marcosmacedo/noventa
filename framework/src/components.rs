use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Clone, Debug)]
pub struct Component {
    pub id: String,
    pub logic_path: Option<String>,
    pub template_path: String,
    pub template_content: String,
}

pub fn scan_components(dir: &Path) -> std::io::Result<Vec<Component>> {
    let mut components_map: HashMap<String, (Option<PathBuf>, Option<(PathBuf, String)>)> = HashMap::new();

    for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() {
            let file_name = path.file_name().unwrap().to_string_lossy();
            let parent_dir = path.parent().unwrap();
            let component_name = parent_dir.strip_prefix(dir).unwrap();
            let component_id = component_name.to_string_lossy().to_string();

            if file_name.ends_with("_logic.py") {
                let entry = components_map.entry(component_id).or_default();
                entry.0 = Some(path.to_path_buf());
            } else if file_name.ends_with(".html") {
                let content = std::fs::read_to_string(path)?;
                let entry = components_map.entry(component_id).or_default();
                entry.1 = Some((path.to_path_buf(), content));
            }
        }
    }

    let components = components_map
        .into_iter()
        .filter_map(|(id, (logic_path, template_data))| {
            template_data.map(|(template_path, template_content)| Component {
                id,
                logic_path: logic_path.map(|p| p.to_string_lossy().into_owned()),
                template_path: template_path.to_string_lossy().into_owned(),
                template_content,
            })
        })
        .collect();

    Ok(components)
}

pub fn scan_single_component(path: &Path, base_path: &Path) -> std::io::Result<Component> {
    let parent_dir = path.parent().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "Component parent directory not found"))?;
    let component_id = parent_dir.strip_prefix(base_path).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Component path is not relative to the components directory"))?.to_str().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "Component path contains invalid UTF-8"))?.to_string();

    let mut logic_path = None;
    let mut template_path = None;
    let mut template_content = None;

    for entry in WalkDir::new(parent_dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() {
            let file_name = path.file_name().unwrap().to_string_lossy();
            if file_name.ends_with("_logic.py") {
                logic_path = Some(path.to_string_lossy().into_owned());
            } else if file_name.ends_with(".html") {
                template_path = Some(path.to_string_lossy().into_owned());
                template_content = Some(std::fs::read_to_string(path)?);
            }
        }
    }

    match (template_path, template_content) {
        (Some(tp), Some(tc)) => Ok(Component {
            id: component_id,
            logic_path,
            template_path: tp,
            template_content: tc,
        }),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Component '{}' is missing a template.html file.", component_id),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_scan_components() {
        let dir = tempdir().unwrap();
        let components_dir = dir.path();

        // Component 1: logic and template
        let comp1_dir = components_dir.join("comp1");
        fs::create_dir(&comp1_dir).unwrap();
        let mut logic1 = File::create(comp1_dir.join("comp1_logic.py")).unwrap();
        logic1.write_all(b"print('hello')").unwrap();
        let mut template1 = File::create(comp1_dir.join("template.html")).unwrap();
        template1.write_all(b"<h1>Comp1</h1>").unwrap();

        // Component 2: template only
        let comp2_dir = components_dir.join("comp2");
        fs::create_dir(&comp2_dir).unwrap();
        let mut template2 = File::create(comp2_dir.join("template.html")).unwrap();
        template2.write_all(b"<h2>Comp2</h2>").unwrap();
        
        // Sub-component
        let sub_comp_dir = components_dir.join("nested/sub");
        fs::create_dir_all(&sub_comp_dir).unwrap();
        let mut sub_template = File::create(sub_comp_dir.join("template.html")).unwrap();
        sub_template.write_all(b"<h3>Sub</h3>").unwrap();

        let components = scan_components(components_dir).unwrap();
        assert_eq!(components.len(), 3);

        let mut component_ids: Vec<_> = components.iter().map(|c| c.id.clone()).collect();
        component_ids.sort();
        assert_eq!(component_ids, vec!["comp1", "comp2", "nested/sub"]);

        for component in components {
            if component.id == "comp1" {
                assert!(component.logic_path.is_some());
                assert!(component.logic_path.unwrap().ends_with("comp1_logic.py"));
                assert_eq!(component.template_content, "<h1>Comp1</h1>");
            } else if component.id == "comp2" {
                assert!(component.logic_path.is_none());
                assert_eq!(component.template_content, "<h2>Comp2</h2>");
            } else if component.id == "nested/sub" {
                assert!(component.logic_path.is_none());
                assert_eq!(component.template_content, "<h3>Sub</h3>");
            }
        }
    }

    #[test]
    fn test_scan_single_component() {
        let dir = tempdir().unwrap();
        let components_dir = dir.path();

        // Create a component with both logic and template
        let comp_dir = components_dir.join("mycomp");
        fs::create_dir(&comp_dir).unwrap();
        let mut logic = File::create(comp_dir.join("mycomp_logic.py")).unwrap();
        logic.write_all(b"print('logic')").unwrap();
        let mut template = File::create(comp_dir.join("template.html")).unwrap();
        template.write_all(b"<div>My Component</div>").unwrap();

        // Test scanning the component
        let component_path = comp_dir.join("template.html");
        let component = scan_single_component(&component_path, components_dir).unwrap();
        
        assert_eq!(component.id, "mycomp");
        assert!(component.logic_path.is_some());
        assert!(component.logic_path.unwrap().contains("mycomp_logic.py"));
        assert!(component.template_path.contains("template.html"));
        assert_eq!(component.template_content, "<div>My Component</div>");
    }

    #[test]
    fn test_scan_single_component_no_logic() {
        let dir = tempdir().unwrap();
        let components_dir = dir.path();

        // Create a component with only template
        let comp_dir = components_dir.join("simple");
        fs::create_dir(&comp_dir).unwrap();
        let mut template = File::create(comp_dir.join("template.html")).unwrap();
        template.write_all(b"<p>Simple</p>").unwrap();

        let component_path = comp_dir.join("template.html");
        let component = scan_single_component(&component_path, components_dir).unwrap();
        
        assert_eq!(component.id, "simple");
        assert!(component.logic_path.is_none());
        assert_eq!(component.template_content, "<p>Simple</p>");
    }

    #[test]
    fn test_scan_single_component_missing_template() {
        let dir = tempdir().unwrap();
        let components_dir = dir.path();

        // Create a component directory with only logic
        let comp_dir = components_dir.join("broken");
        fs::create_dir(&comp_dir).unwrap();
        let mut logic = File::create(comp_dir.join("broken_logic.py")).unwrap();
        logic.write_all(b"print('broken')").unwrap();

        let component_path = comp_dir.join("broken_logic.py");
        let result = scan_single_component(&component_path, components_dir);
        assert!(result.is_err());
    }
}