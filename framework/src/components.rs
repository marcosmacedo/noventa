use std::fs;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Clone)]
pub struct Component {
    pub id: String,
    pub code_path: Option<String>,
}

pub fn scan_components(dir: &Path) -> std::io::Result<Vec<Component>> {
    let mut components = Vec::new();
    for entry in WalkDir::new(dir).min_depth(1).max_depth(1).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            let component_name = path
                .strip_prefix(dir)
                .unwrap()
                .to_string_lossy()
                .replace("/", ".");

            let mut html_files = Vec::new();
            let mut py_files = Vec::new();

            for sub_entry in fs::read_dir(path)? {
                let sub_entry = sub_entry?;
                let sub_path = sub_entry.path();
                if sub_path.is_file() {
                    if sub_path.extension().and_then(|s| s.to_str()) == Some("html") {
                        html_files.push(sub_path);
                    } else if sub_path.extension().and_then(|s| s.to_str()) == Some("py") {
                        py_files.push(sub_path);
                    }
                }
            }

            if html_files.len() > 1 {
                log::error!("Component '{}' has more than one .html file. Skipping.", component_name);
                continue;
            }

            if py_files.len() > 1 {
                log::error!("Component '{}' has more than one .py file. Skipping.", component_name);
                continue;
            }

            if html_files.into_iter().next().is_some() {
                let component = Component {
                    id: component_name.clone(),
                    code_path: py_files.into_iter().next().map(|p| p.to_string_lossy().into_owned()),
                };
                components.push(component);
            }
        }
    }
    Ok(components)
}