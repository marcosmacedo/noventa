// framework/src/disco/interactive_tools/models.rs
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone)]
pub struct InteractiveTool {
    pub name: String,
    pub description: String,
    #[serde(rename = "initial_step")]
    pub initial_step: String,
    pub steps: HashMap<String, Step>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Step {
    pub text: String,
    pub options: Option<Vec<OptionDef>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OptionDef {
    pub label: String,
    #[serde(rename = "next_step")]
    pub next_step: String,
}