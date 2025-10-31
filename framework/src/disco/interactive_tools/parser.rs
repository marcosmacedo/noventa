// framework/src/disco/interactive_tools/parser.rs
use crate::disco::interactive_tools::models::InteractiveTool;
use once_cell::sync::Lazy;
use std::collections::HashMap;

// const API_INTEGRATION_HELPER: &str =
//     include_str!("tools_yaml/api_integration_helper.yaml");
// const COMPONENT_GENERATOR: &str =
//     include_str!("tools_yaml/component_generator.yaml");
// const CONFIG_EXPLAINER: &str =
//     include_str!("tools_yaml/config_explainer.yaml");
const DATABASE_MIGRATION_HELPER: &str =
    include_str!("tools_yaml/database_migration_helper.yaml");
// const DYNAMIC_ROUTING_EXPLAINER: &str =
//     include_str!("tools_yaml/dynamic_routing_explainer.yaml");
// const NEW_PAGE_CREATOR: &str =
//     include_str!("tools_yaml/new_page_creator.yaml");
// const ONBOARDING_GUIDE: &str =
//     include_str!("tools_yaml/onboarding_guide.yaml");
// const STATIC_FILE_GUIDE: &str =
//     include_str!("tools_yaml/static_file_guide.yaml");

static TOOLS: Lazy<HashMap<String, InteractiveTool>> = Lazy::new(|| {
    let mut tools = HashMap::new();
    let tool_files = vec![
        // API_INTEGRATION_HELPER,
        // COMPONENT_GENERATOR,
        // CONFIG_EXPLAINER,
        DATABASE_MIGRATION_HELPER,
        // DYNAMIC_ROUTING_EXPLAINER,
        // NEW_PAGE_CREATOR,
        // ONBOARDING_GUIDE,
        // STATIC_FILE_GUIDE,
    ];

    for content in tool_files {
        let tool: InteractiveTool = serde_yaml::from_str(content)
            .expect("Failed to parse tool file");
        tools.insert(tool.name.clone(), tool);
    }
    tools
});

pub fn load_tools() -> &'static HashMap<String, InteractiveTool> {
    &TOOLS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_tools() {
        let tools = load_tools();
        assert!(!tools.is_empty());
        // Since only DATABASE_MIGRATION_HELPER is included
        assert!(tools.contains_key("database_migration_helper"));
    }
}