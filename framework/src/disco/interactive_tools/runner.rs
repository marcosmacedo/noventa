// framework/src/disco/interactive_tools/runner.rs
use crate::disco::interactive_tools::models::{InteractiveTool, Step};
use crate::disco::interactive_tools::session::SessionManager;
use std::collections::HashMap;

pub struct ToolRunner {
    pub tools: HashMap<String, InteractiveTool>,
    session_manager: SessionManager,
}

impl ToolRunner {
    pub fn new(tools: HashMap<String, InteractiveTool>, session_manager: SessionManager) -> Self {
        Self {
            tools,
            session_manager,
        }
    }

    pub fn run_tool(&self, tool_name: &str, user_input: Option<usize>) -> String {
        let tool = match self.tools.get(tool_name) {
            Some(t) => t,
            None => return "Unknown tool".to_string(),
        };

        let session_existed = self.session_manager.get_session().map_or(false, |s| s.tool_name == tool_name);

        let mut session = match self.session_manager.get_session() {
            Some(s) if s.tool_name == tool_name => s,
            _ => {
                self.session_manager.end_session();
                self.session_manager
                    .create_session(tool_name, &tool.initial_step)
            }
        };

        if session_existed {
            if let Some(input_index) = user_input {
                let current_step = tool.steps.get(&session.current_step).unwrap();
                if let Some(options) = &current_step.options {
                    if input_index > 0 {
                        if let Some(selected_option) = options.get(input_index - 1) {
                            if selected_option.next_step == "[END]" {
                                self.session_manager.end_session();
                                return "Session ended.".to_string();
                            }
                            session.current_step = selected_option.next_step.clone();
                            self.session_manager.update_session(&session.current_step);
                        } else {
                            return "Invalid option.".to_string();
                        }
                    } else {
                        return "Invalid option.".to_string();
                    }
                } else {
                    self.session_manager.end_session();
                    return "Session ended.".to_string();
                }
            }
        }

        let step_def = tool.steps.get(&session.current_step).unwrap();

        let response = self.format_step(step_def, tool_name);
        if step_def.options.is_none() {
            self.session_manager.end_session();
        }

        response
    }

    fn format_step(&self, step: &Step, tool_name: &str) -> String {
        let mut response = step.text.clone();
        if let Some(options) = &step.options {
            for (i, option) in options.iter().enumerate() {
                response.push_str(&format!("\n{}. {}", i + 1, option.label));
            }
            response.push_str(&format!("\n\nReply calling the tool ({}) and passing your numerical option in user_input", tool_name));
        }
        response
    }
}