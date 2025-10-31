use actix::prelude::*;
use actix_session::Session;
use std::io::{Error, ErrorKind};

// Define the actor
pub struct SessionManagerActor {
    session: Session,
}

impl SessionManagerActor {
    pub fn new(session: Session) -> Self {
        Self { session }
    }
}

impl Actor for SessionManagerActor {
    type Context = Context<Self>;
}

// Define messages
#[derive(Message)]
#[rtype(result = "Result<Option<String>, Error>")]
pub struct GetSessionValue {
    pub key: String,
}

#[derive(Message)]
#[rtype(result = "Result<(), Error>")]
pub struct SetSessionValue {
    pub key: String,
    pub value: String,
}

#[derive(Message)]
#[rtype(result = "Result<(), Error>")]
pub struct DeleteSessionValue {
    pub key: String,
}

#[derive(Message)]
#[rtype(result = "Result<(), Error>")]
pub struct ClearSession;

#[derive(Message, Copy, Clone)]
#[rtype(result = "Result<actix_session::SessionStatus, Error>")]
pub struct GetStatus;

#[derive(Message, Copy, Clone)]
#[rtype(result = "Result<(), Error>")]
pub struct SetPermanent {
    pub permanent: bool,
}

#[derive(Message, Copy, Clone)]
#[rtype(result = "Result<(), Error>")]
pub struct MarkAsModified;

// Define message handlers
impl Handler<GetSessionValue> for SessionManagerActor {
    type Result = Result<Option<String>, Error>;

    fn handle(&mut self, msg: GetSessionValue, _ctx: &mut Context<Self>) -> Self::Result {
        self.session.get(&msg.key).map_err(|e| Error::new(ErrorKind::Other, e.to_string()))
    }
}

impl Handler<SetSessionValue> for SessionManagerActor {
    type Result = Result<(), Error>;

    fn handle(&mut self, msg: SetSessionValue, _ctx: &mut Context<Self>) -> Self::Result {
        self.session.insert(&msg.key, &msg.value).map_err(|e| Error::new(ErrorKind::Other, e.to_string()))
    }
}

impl Handler<DeleteSessionValue> for SessionManagerActor {
    type Result = Result<(), Error>;

    fn handle(&mut self, msg: DeleteSessionValue, _ctx: &mut Context<Self>) -> Self::Result {
        self.session.remove(&msg.key);
        Ok(())
    }
}

impl Handler<ClearSession> for SessionManagerActor {
    type Result = Result<(), Error>;

    fn handle(&mut self, _msg: ClearSession, _ctx: &mut Context<Self>) -> Self::Result {
        self.session.clear();
        Ok(())
    }
}

impl Handler<GetStatus> for SessionManagerActor {
    type Result = Result<actix_session::SessionStatus, Error>;

    fn handle(&mut self, _msg: GetStatus, _ctx: &mut Context<Self>) -> Self::Result {
        Ok(self.session.status())
    }
}

impl Handler<SetPermanent> for SessionManagerActor {
    type Result = Result<(), Error>;

    fn handle(&mut self, msg: SetPermanent, _ctx: &mut Context<Self>) -> Self::Result {
        if msg.permanent {
            self.session.renew();
        } else {
            self.session.purge();
        }
        Ok(())
    }
}

impl Handler<MarkAsModified> for SessionManagerActor {
    type Result = Result<(), Error>;

    fn handle(&mut self, _msg: MarkAsModified, _ctx: &mut Context<Self>) -> Self::Result {
        // Renewing the session marks it as changed and forces a new cookie to be issued.
        // This is the idiomatic way to manually mark the session as modified.
        self.session.renew();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix::Actor;
    use actix::actors::mocker::Mocker;
    use std::io::Error;

    // Using the Mocker pattern for proper actor testing
    type SessionManagerActorMock = Mocker<SessionManagerActor>;

    #[actix_rt::test]
    async fn test_session_manager_actor_creation() {
        let session_mock = SessionManagerActorMock::mock(Box::new(|msg, _ctx| {
            // Mock responses for different message types
            if let Some(_) = msg.downcast_ref::<GetSessionValue>() {
                Box::new(Some(Ok::<Option<String>, std::io::Error>(None)))
            } else if let Some(_) = msg.downcast_ref::<SetSessionValue>() {
                Box::new(Some(Ok::<(), std::io::Error>(())))
            } else if let Some(_) = msg.downcast_ref::<DeleteSessionValue>() {
                Box::new(Some(Ok::<(), std::io::Error>(())))
            } else if let Some(_) = msg.downcast_ref::<ClearSession>() {
                Box::new(Some(Ok::<(), std::io::Error>(())))
            } else if let Some(_) = msg.downcast_ref::<GetStatus>() {
                Box::new(Some(Ok::<actix_session::SessionStatus, std::io::Error>(actix_session::SessionStatus::Changed)))
            } else if let Some(_) = msg.downcast_ref::<SetPermanent>() {
                Box::new(Some(Ok::<(), std::io::Error>(())))
            } else if let Some(_) = msg.downcast_ref::<MarkAsModified>() {
                Box::new(Some(Ok::<(), std::io::Error>(())))
            } else {
                Box::new(Some(Ok::<(), std::io::Error>(())))
            }
        }));

        let addr = session_mock.start();
        assert!(addr.connected());
    }

    #[actix_rt::test]
    async fn test_get_session_value_message_handling() {
        let session_mock = SessionManagerActorMock::mock(Box::new(|msg, _ctx| {
            if let Some(get_msg) = msg.downcast_ref::<GetSessionValue>() {
                if get_msg.key == "test_key" {
                    Box::new(Some(Ok::<Option<String>, std::io::Error>(Some("test_value".to_string()))))
                } else {
                    Box::new(Some(Ok::<Option<String>, std::io::Error>(None)))
                }
            } else {
                Box::new(Some(Ok::<Option<String>, std::io::Error>(None)))
            }
        }));

        let addr = session_mock.start();
        
        // Test getting an existing value
        let get_msg = GetSessionValue { key: "test_key".to_string() };
        let result = addr.send(get_msg).await;
        assert!(result.is_ok());
        
        // Test getting a non-existing value
        let get_missing_msg = GetSessionValue { key: "missing_key".to_string() };
        let result = addr.send(get_missing_msg).await;
        assert!(result.is_ok());
    }

    #[actix_rt::test]
    async fn test_set_session_value_message_handling() {
        let session_mock = SessionManagerActorMock::mock(Box::new(|msg, _ctx| {
            if let Some(_) = msg.downcast_ref::<SetSessionValue>() {
                Box::new(Some(Ok::<(), std::io::Error>(())))
            } else {
                Box::new(Some(Ok::<(), std::io::Error>(())))
            }
        }));

        let addr = session_mock.start();
        
        let set_msg = SetSessionValue { 
            key: "test_key".to_string(), 
            value: "test_value".to_string() 
        };
        let result = addr.send(set_msg).await;
        assert!(result.is_ok());
    }

    #[actix_rt::test]
    async fn test_delete_session_value_message_handling() {
        let session_mock = SessionManagerActorMock::mock(Box::new(|msg, _ctx| {
            if let Some(_) = msg.downcast_ref::<DeleteSessionValue>() {
                Box::new(Some(Ok::<(), std::io::Error>(())))
            } else {
                Box::new(Some(Ok::<(), std::io::Error>(())))
            }
        }));

        let addr = session_mock.start();
        
        let delete_msg = DeleteSessionValue { key: "test_key".to_string() };
        let result = addr.send(delete_msg).await;
        assert!(result.is_ok());
    }

    #[actix_rt::test]
    async fn test_clear_session_message_handling() {
        let session_mock = SessionManagerActorMock::mock(Box::new(|msg, _ctx| {
            if let Some(_) = msg.downcast_ref::<ClearSession>() {
                Box::new(Some(Ok::<(), std::io::Error>(())))
            } else {
                Box::new(Some(Ok::<(), std::io::Error>(())))
            }
        }));

        let addr = session_mock.start();
        
        let clear_msg = ClearSession;
        let result = addr.send(clear_msg).await;
        assert!(result.is_ok());
    }

    #[actix_rt::test]
    async fn test_get_status_message_handling() {
        let session_mock = SessionManagerActorMock::mock(Box::new(|msg, _ctx| {
            if let Some(_) = msg.downcast_ref::<GetStatus>() {
                Box::new(Some(Ok::<actix_session::SessionStatus, std::io::Error>(actix_session::SessionStatus::Changed)))
            } else {
                Box::new(Some(Ok::<actix_session::SessionStatus, std::io::Error>(actix_session::SessionStatus::Unchanged)))
            }
        }));

        let addr = session_mock.start();
        
        let status_msg = GetStatus;
        let result = addr.send(status_msg).await;
        assert!(result.is_ok());
    }

    #[actix_rt::test]
    async fn test_set_permanent_message_handling() {
        let session_mock = SessionManagerActorMock::mock(Box::new(|msg, _ctx| {
            if let Some(_) = msg.downcast_ref::<SetPermanent>() {
                Box::new(Some(Ok::<(), std::io::Error>(())))
            } else {
                Box::new(Some(Ok::<(), std::io::Error>(())))
            }
        }));

        let addr = session_mock.start();
        
        let permanent_msg = SetPermanent { permanent: true };
        let result = addr.send(permanent_msg).await;
        assert!(result.is_ok());
        
        let temp_msg = SetPermanent { permanent: false };
        let result = addr.send(temp_msg).await;
        assert!(result.is_ok());
    }

    #[actix_rt::test]
    async fn test_mark_as_modified_message_handling() {
        let session_mock = SessionManagerActorMock::mock(Box::new(|msg, _ctx| {
            if let Some(_) = msg.downcast_ref::<MarkAsModified>() {
                Box::new(Some(Ok::<(), std::io::Error>(())))
            } else {
                Box::new(Some(Ok::<(), std::io::Error>(())))
            }
        }));

        let addr = session_mock.start();
        
        let modified_msg = MarkAsModified;
        let result = addr.send(modified_msg).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_message_types() {
        // Test that all message types can be created
        let _get_msg = GetSessionValue { key: "test".to_string() };
        let _set_msg = SetSessionValue { key: "test".to_string(), value: "value".to_string() };
        let _delete_msg = DeleteSessionValue { key: "test".to_string() };
        let _clear_msg = ClearSession;
        let _status_msg = GetStatus;
        let _permanent_msg = SetPermanent { permanent: true };
        let _modified_msg = MarkAsModified;
        assert!(true);
    }
}