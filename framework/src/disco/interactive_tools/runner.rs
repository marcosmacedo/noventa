// framework/src/disco/interactive_tools/runner.rs
use crate::disco::interactive_tools::models::{InteractiveTool, Step};
use crate::disco::interactive_tools::session::{Session, SessionManager};
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

    pub fn run_tool(&self, user_id: &str, tool_name: &str, user_input: Option<usize>) -> String {
        let tool = match self.tools.get(tool_name) {
            Some(t) => t,
            None => return "Unknown tool".to_string(),
        };

        // Get or create a session
        let mut session = self.session_manager.get_session(user_id)
            .unwrap_or_else(|| self.session_manager.create_session(user_id, tool_name, &tool.initial_step));

        // If there is user input, process it to find the next step
        if let Some(input_index) = user_input {
            let current_step = tool.steps.get(&session.current_step).unwrap();
            if let Some(options) = &current_step.options {
                if let Some(selected_option) = options.get(input_index - 1) {
                    if selected_option.next_step == "[END]" {
                        self.session_manager.end_session(user_id);
                        return "Session ended.".to_string();
                    }
                    // Update session to the next step
                    session.current_step = selected_option.next_step.clone();
                    self.session_manager.update_session(user_id, &session.current_step);
                } else {
                    return "Invalid option.".to_string();
                }
            } else { // End of a branch with no options
                self.session_manager.end_session(user_id);
                return "Session ended.".to_string();
            }
        }

        // Get the step definition for the current (or newly updated) step
        let step_def = tool.steps.get(&session.current_step).unwrap();

        // Format the response. If there are no options, this is a final step.
        let response = self.format_step(step_def, tool_name);
        if step_def.options.is_none() {
            self.session_manager.end_session(user_id);
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