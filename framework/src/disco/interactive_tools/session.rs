// framework/src/disco/interactive_tools/session.rs
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct Session {
    pub current_step: String,
    pub tool_name: String,
}

pub struct SessionManager {
    session: Arc<Mutex<Option<Session>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            session: Arc::new(Mutex::new(None)),
        }
    }

    pub fn get_session(&self) -> Option<Session> {
        self.session.lock().unwrap().clone()
    }

    pub fn create_session(&self, tool_name: &str, initial_step: &str) -> Session {
        let mut session_guard = self.session.lock().unwrap();
        let new_session = Session {
            current_step: initial_step.to_string(),
            tool_name: tool_name.to_string(),
        };
        *session_guard = Some(new_session.clone());
        new_session
    }

    pub fn update_session(&self, next_step: &str) {
        let mut session_guard = self.session.lock().unwrap();
        if let Some(session) = session_guard.as_mut() {
            session.current_step = next_step.to_string();
        }
    }

    pub fn end_session(&self) {
        *self.session.lock().unwrap() = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_manager_new() {
        let manager = SessionManager::new();
        assert!(manager.get_session().is_none());
    }

    #[test]
    fn test_session_manager_create_session() {
        let manager = SessionManager::new();
        let session = manager.create_session("test_tool", "step1");
        assert_eq!(session.tool_name, "test_tool");
        assert_eq!(session.current_step, "step1");
        assert_eq!(manager.get_session().unwrap().tool_name, "test_tool");
    }

    #[test]
    fn test_session_manager_update_session() {
        let manager = SessionManager::new();
        manager.create_session("test_tool", "step1");
        manager.update_session("step2");
        assert_eq!(manager.get_session().unwrap().current_step, "step2");
    }

    #[test]
    fn test_session_manager_end_session() {
        let manager = SessionManager::new();
        manager.create_session("test_tool", "step1");
        assert!(manager.get_session().is_some());
        manager.end_session();
        assert!(manager.get_session().is_none());
    }
}