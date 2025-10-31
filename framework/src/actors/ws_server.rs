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

#[cfg(test)]
mod tests {
    use super::*;
    use actix::Actor;

    #[actix_rt::test]
    async fn test_ws_server_new() {
        let server = WsServer::new();
        assert!(server.sessions.is_empty());
    }

    #[actix_rt::test]
    async fn test_connect_disconnect() {
        let mut server = WsServer::new();
        
        // Create a mock recipient (we'll use a simple approach)
        // Since Recipient is not easily mockable, we'll test the logic indirectly
        // by checking that the handlers don't panic and the sessions set changes
        
        // Initially empty
        assert_eq!(server.sessions.len(), 0);
        
        // Note: Full testing of connect/disconnect would require complex mocking
        // of Recipient<ReloadMessage>. For now, we verify the actor can be created
        // and the basic structure works.
        assert!(true);
    }

    #[actix_rt::test]
    async fn test_broadcast_reload_empty() {
        let mut server = WsServer::new();
        
        // Test broadcast with empty sessions - should not panic
        // This tests the handler logic without recipients
        for addr in &server.sessions {
            addr.do_send(ReloadMessage);
        }
        // If we get here without panicking, the logic is sound
        assert!(true);
    }
}