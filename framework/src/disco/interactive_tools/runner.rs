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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disco::interactive_tools::models::{InteractiveTool, Step, OptionDef};
    use std::collections::HashMap;

    fn create_test_tool() -> InteractiveTool {
        let mut steps = HashMap::new();
        steps.insert("start".to_string(), Step {
            text: "Welcome to the test tool".to_string(),
            options: Some(vec![
                OptionDef {
                    label: "Option 1".to_string(),
                    next_step: "step1".to_string(),
                },
                OptionDef {
                    label: "End".to_string(),
                    next_step: "[END]".to_string(),
                },
            ]),
        });
        steps.insert("step1".to_string(), Step {
            text: "You chose option 1".to_string(),
            options: None,
        });

        InteractiveTool {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            initial_step: "start".to_string(),
            steps,
        }
    }

    #[test]
    fn test_tool_runner_new() {
        let tools = HashMap::new();
        let session_manager = SessionManager::new();
        let runner = ToolRunner::new(tools, session_manager);
        assert!(runner.tools.is_empty());
    }

    #[test]
    fn test_run_tool_unknown() {
        let tools = HashMap::new();
        let session_manager = SessionManager::new();
        let runner = ToolRunner::new(tools, session_manager);
        let result = runner.run_tool("unknown", None);
        assert_eq!(result, "Unknown tool");
    }

    #[test]
    fn test_run_tool_initial_step() {
        let mut tools = HashMap::new();
        tools.insert("test_tool".to_string(), create_test_tool());
        let session_manager = SessionManager::new();
        let runner = ToolRunner::new(tools, session_manager);
        let result = runner.run_tool("test_tool", None);
        assert!(result.contains("Welcome to the test tool"));
        assert!(result.contains("1. Option 1"));
        assert!(result.contains("2. End"));
    }

    #[test]
    fn test_run_tool_with_option() {
        let mut tools = HashMap::new();
        tools.insert("test_tool".to_string(), create_test_tool());
        let session_manager = SessionManager::new();
        let runner = ToolRunner::new(tools, session_manager);
        
        // Start session
        runner.run_tool("test_tool", None);
        
        // Choose option 1
        let result = runner.run_tool("test_tool", Some(1));
        assert_eq!(result, "You chose option 1");
    }

    #[test]
    fn test_run_tool_end_option() {
        let mut tools = HashMap::new();
        tools.insert("test_tool".to_string(), create_test_tool());
        let session_manager = SessionManager::new();
        let runner = ToolRunner::new(tools, session_manager);
        
        // Start session
        runner.run_tool("test_tool", None);
        
        // Choose end option
        let result = runner.run_tool("test_tool", Some(2));
        assert_eq!(result, "Session ended.");
    }

    #[test]
    fn test_run_tool_invalid_option() {
        let mut tools = HashMap::new();
        tools.insert("test_tool".to_string(), create_test_tool());
        let session_manager = SessionManager::new();
        let runner = ToolRunner::new(tools, session_manager);
        
        // Start session
        runner.run_tool("test_tool", None);
        
        // Choose invalid option
        let result = runner.run_tool("test_tool", Some(10));
        assert_eq!(result, "Invalid option.");
    }

    #[test]
    fn test_format_step_with_options() {
        let tools = HashMap::new();
        let session_manager = SessionManager::new();
        let runner = ToolRunner::new(tools, session_manager);
        
        let step = Step {
            text: "Choose an option".to_string(),
            options: Some(vec![
                OptionDef {
                    label: "Yes".to_string(),
                    next_step: "yes".to_string(),
                },
                OptionDef {
                    label: "No".to_string(),
                    next_step: "no".to_string(),
                },
            ]),
        };
        
        let result = runner.format_step(&step, "test_tool");
        assert!(result.contains("Choose an option"));
        assert!(result.contains("1. Yes"));
        assert!(result.contains("2. No"));
        assert!(result.contains("Reply calling the tool (test_tool)"));
    }

    #[test]
    fn test_format_step_without_options() {
        let tools = HashMap::new();
        let session_manager = SessionManager::new();
        let runner = ToolRunner::new(tools, session_manager);
        
        let step = Step {
            text: "Final message".to_string(),
            options: None,
        };
        
        let result = runner.format_step(&step, "test_tool");
        assert_eq!(result, "Final message");
    }
}