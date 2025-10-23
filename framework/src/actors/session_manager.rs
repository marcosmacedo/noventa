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