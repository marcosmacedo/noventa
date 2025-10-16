use super::interpreter::RenderComponent;
use actix::prelude::*;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Error, ErrorKind};

pub struct InterpreterManager {
    recipients: Vec<Recipient<RenderComponent>>,
    next: usize,
}

impl InterpreterManager {
    pub fn new(recipients: Vec<Recipient<RenderComponent>>) -> Self {
        InterpreterManager {
            recipients,
            next: 0,
        }
    }
}

impl Actor for InterpreterManager {
    type Context = Context<Self>;
}

impl Handler<RenderComponent> for InterpreterManager {
    type Result = ResponseFuture<Result<HashMap<String, Value>, Error>>;

    fn handle(&mut self, msg: RenderComponent, _ctx: &mut Self::Context) -> Self::Result {
        let recipient = self.recipients[self.next].clone();
        self.next = (self.next + 1) % self.recipients.len();

        Box::pin(async move {
            match recipient.send(msg).await {
                Ok(res) => res,
                Err(e) => {
                    log::error!("Mailbox error calling interpreter actor: {}", e);
                    Err(Error::new(ErrorKind::Other, e.to_string()))
                }
            }
        })
    }
}