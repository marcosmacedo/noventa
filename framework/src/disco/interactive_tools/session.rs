// framework/src/disco/interactive_tools/session.rs
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct Session {
    pub tool_name: String,
    pub current_step: String,
    pub user_id: String, // A unique identifier for the user
}

pub struct SessionManager {
    sessions: Arc<Mutex<HashMap<String, Session>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get_session(&self, user_id: &str) -> Option<Session> {
        let sessions = self.sessions.lock().unwrap();
        sessions.get(user_id).cloned()
    }

    pub fn create_session(&self, user_id: &str, tool_name: &str, initial_step: &str) -> Session {
        let mut sessions = self.sessions.lock().unwrap();
        let session = Session {
            tool_name: tool_name.to_string(),
            current_step: initial_step.to_string(),
            user_id: user_id.to_string(),
        };
        sessions.insert(user_id.to_string(), session.clone());
        session
    }

    pub fn update_session(&self, user_id: &str, next_step: &str) {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(user_id) {
            session.current_step = next_step.to_string();
        }
    }

    pub fn end_session(&self, user_id: &str) {
        let mut sessions = self.sessions.lock().unwrap();
        sessions.remove(user_id);
    }
}