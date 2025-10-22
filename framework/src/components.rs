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