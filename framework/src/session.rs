use actix_session::storage::{
    CookieSessionStore, LoadError, RedisSessionStore, SaveError, SessionKey, SessionStore,
    UpdateError,
};
use actix_web::cookie::time::Duration;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct InMemoryBackend {
    sessions: Arc<Mutex<HashMap<String, HashMap<String, String>>>>,
}

impl InMemoryBackend {
    pub fn new() -> Self {
        InMemoryBackend {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl SessionStore for InMemoryBackend {
    async fn load(&self, session_key: &SessionKey) -> Result<Option<HashMap<String, String>>, LoadError> {
        let key = session_key.as_ref();
        let sessions = self.sessions.lock().unwrap();
        Ok(sessions.get(key).cloned())
    }

    async fn save(
        &self,
        session_state: HashMap<String, String>,
        _ttl: &Duration,
    ) -> Result<SessionKey, SaveError> {
        let session_key = actix_session::storage::generate_session_key();
        let key = session_key.as_ref().to_string();
        let mut sessions = self.sessions.lock().unwrap();
        sessions.insert(key, session_state);
        Ok(session_key)
    }

    async fn update(
        &self,
        session_key: SessionKey,
        session_state: HashMap<String, String>,
        _ttl: &Duration,
    ) -> Result<SessionKey, UpdateError> {
        let key = session_key.as_ref().to_string();
        let mut sessions = self.sessions.lock().unwrap();
        sessions.insert(key, session_state);
        Ok(session_key)
    }

    async fn update_ttl(&self, _session_key: &SessionKey, _ttl: &Duration) -> Result<(), anyhow::Error> {
        // TTL is not managed in this simple in-memory backend
        Ok(())
    }

    async fn delete(&self, session_key: &SessionKey) -> Result<(), anyhow::Error> {
        let key = session_key.as_ref();
        let mut sessions = self.sessions.lock().unwrap();
        sessions.remove(key);
        Ok(())
    }
}


#[derive(Clone)]
pub enum RuntimeSessionStore {
    Cookie(Arc<CookieSessionStore>),
    InMemory(InMemoryBackend),
    Redis(RedisSessionStore),
}

impl RuntimeSessionStore {
    pub fn new_inmemory() -> Self {
        RuntimeSessionStore::InMemory(InMemoryBackend::new())
    }
}

impl SessionStore for RuntimeSessionStore {
    async fn load(&self, session_key: &SessionKey) -> Result<Option<HashMap<String, String>>, LoadError> {
        match self {
            RuntimeSessionStore::Cookie(s) => s.load(session_key).await,
            RuntimeSessionStore::InMemory(s) => s.load(session_key).await,
            RuntimeSessionStore::Redis(s) => s.load(session_key).await,
        }
    }

    async fn save(
        &self,
        session_state: HashMap<String, String>,
        ttl: &actix_web::cookie::time::Duration,
    ) -> Result<SessionKey, SaveError> {
        match self {
            RuntimeSessionStore::Cookie(s) => s.save(session_state, ttl).await,
            RuntimeSessionStore::InMemory(s) => s.save(session_state, ttl).await,
            RuntimeSessionStore::Redis(s) => s.save(session_state, ttl).await,
        }
    }

    async fn update(
        &self,
        session_key: SessionKey,
        session_state: HashMap<String, String>,
        ttl: &actix_web::cookie::time::Duration,
    ) -> Result<SessionKey, UpdateError> {
        match self {
            RuntimeSessionStore::Cookie(s) => s.update(session_key, session_state, ttl).await,
            RuntimeSessionStore::InMemory(s) => s.update(session_key, session_state, ttl).await,
            RuntimeSessionStore::Redis(s) => s.update(session_key, session_state, ttl).await,
        }
    }

    async fn update_ttl(&self, session_key: &SessionKey, ttl: &actix_web::cookie::time::Duration) -> Result<(), anyhow::Error> {
        match self {
            RuntimeSessionStore::Cookie(s) => s.update_ttl(session_key, ttl).await,
            RuntimeSessionStore::InMemory(s) => s.update_ttl(session_key, ttl).await,
            RuntimeSessionStore::Redis(s) => s.update_ttl(session_key, ttl).await,
        }
    }

    async fn delete(&self, session_key: &SessionKey) -> Result<(), anyhow::Error> {
        match self {
            RuntimeSessionStore::Cookie(s) => s.delete(session_key).await,
            RuntimeSessionStore::InMemory(s) => s.delete(session_key).await,
            RuntimeSessionStore::Redis(s) => s.delete(session_key).await,
        }
    }
}