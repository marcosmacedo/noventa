use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Clone)]
pub struct Component {
    pub id: String,
    pub template_path: String,
}

fn find_components_recursive(
    dir: &Path,
    base_dir: &Path,
    components: &mut HashMap<String, Component>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let component_name = path
                .strip_prefix(base_dir)
                .unwrap()
                .to_str()
                .unwrap()
                .replace("/", ".");

            let template_path = path.join(path.file_name().unwrap()).with_extension("html");

            if template_path.exists() {
                let component = Component {
                    id: component_name.clone(),
                    template_path: template_path.to_str().unwrap().to_string(),
                };
                components.insert(component_name, component);
            }
            find_components_recursive(&path, base_dir, components)?;
        }
    }
    Ok(())
}

pub fn scan_components(dir: &Path) -> std::io::Result<HashMap<String, Component>> {
    let mut components = HashMap::new();
    find_components_recursive(dir, dir, &mut components)?;
    Ok(components)
}