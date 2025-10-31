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

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::cookie::time::Duration;

    #[actix_rt::test]
    async fn test_in_memory_backend() {
        let backend = InMemoryBackend::new();
        let ttl = Duration::days(1);

        // Save a new session
        let mut session_state = HashMap::new();
        session_state.insert("key1".to_string(), "value1".to_string());
        let session_key = backend.save(session_state.clone(), &ttl).await.unwrap();

        // Load the session
        let loaded_session = backend.load(&session_key).await.unwrap().unwrap();
        assert_eq!(loaded_session, session_state);

        // Update the session
        let mut updated_session_state = session_state.clone();
        updated_session_state.insert("key2".to_string(), "value2".to_string());
        let session_key_for_update = SessionKey::try_from(session_key.as_ref().to_string()).unwrap();
        backend
            .update(
                session_key_for_update,
                updated_session_state.clone(),
                &ttl,
            )
            .await
            .unwrap();

        // Load the updated session
        let loaded_updated_session = backend.load(&session_key).await.unwrap().unwrap();
        assert_eq!(loaded_updated_session, updated_session_state);

        // Delete the session
        backend.delete(&session_key).await.unwrap();
        let deleted_session = backend.load(&session_key).await.unwrap();
        assert!(deleted_session.is_none());
    }
    #[actix_rt::test]
    async fn test_in_memory_backend_ttl_and_delete() {
        let backend = InMemoryBackend::new();
        let ttl = Duration::days(1);

        // Save a new session
        let mut session_state = HashMap::new();
        session_state.insert("key1".to_string(), "value1".to_string());
        let session_key = backend.save(session_state.clone(), &ttl).await.unwrap();

        // Test update_ttl
        assert!(backend.update_ttl(&session_key, &ttl).await.is_ok());

        // Test delete
        backend.delete(&session_key).await.unwrap();
        let deleted_session = backend.load(&session_key).await.unwrap();
        assert!(deleted_session.is_none());
    }

    #[actix_rt::test]
    async fn test_runtime_session_store_in_memory() {
        let backend = InMemoryBackend::new();
        let store = RuntimeSessionStore::InMemory(backend);
        let ttl = Duration::days(1);

        // Save a new session
        let mut session_state = HashMap::new();
        session_state.insert("key1".to_string(), "value1".to_string());
        let session_key = store.save(session_state.clone(), &ttl).await.unwrap();

        // Load the session
        let loaded_session = store.load(&session_key).await.unwrap().unwrap();
        assert_eq!(loaded_session, session_state);

        // Update the session
        let mut updated_session_state = session_state.clone();
        updated_session_state.insert("key2".to_string(), "value2".to_string());
        let session_key_for_update = SessionKey::try_from(session_key.as_ref().to_string()).unwrap();
        store
            .update(
                session_key_for_update,
                updated_session_state.clone(),
                &ttl,
            )
            .await
            .unwrap();

        // Load the updated session
        let loaded_updated_session = store.load(&session_key).await.unwrap().unwrap();
        assert_eq!(loaded_updated_session, updated_session_state);

        // Test update_ttl
        assert!(store.update_ttl(&session_key, &ttl).await.is_ok());

        // Delete the session
        store.delete(&session_key).await.unwrap();
        let deleted_session = store.load(&session_key).await.unwrap();
        assert!(deleted_session.is_none());
    }
}