// framework/src/disco/interactive_tools/session.rs
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct Session {
    pub current_step: String,
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

    pub fn create_session(&self, initial_step: &str) -> Session {
        let mut session_guard = self.session.lock().unwrap();
        let new_session = Session {
            current_step: initial_step.to_string(),
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