use actix::prelude::*;
use std::collections::HashSet;
use crate::actors::dev_websockets::ReloadMessage;

#[derive(Message)]
#[rtype(result = "()")]
pub struct Connect {
    pub addr: Recipient<ReloadMessage>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub addr: Recipient<ReloadMessage>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct BroadcastReload;

pub struct WsServer {
    sessions: HashSet<Recipient<ReloadMessage>>,
}

impl WsServer {
    pub fn new() -> Self {
        WsServer {
            sessions: HashSet::new(),
        }
    }
}

impl Actor for WsServer {
    type Context = Context<Self>;
}

impl Handler<Connect> for WsServer {
    type Result = ();

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) {
        self.sessions.insert(msg.addr);
    }
}

impl Handler<Disconnect> for WsServer {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        self.sessions.remove(&msg.addr);
    }
}

impl Handler<BroadcastReload> for WsServer {
    type Result = ();

    fn handle(&mut self, _msg: BroadcastReload, _: &mut Context<Self>) {
        for addr in &self.sessions {
            addr.do_send(ReloadMessage);
        }
    }
}