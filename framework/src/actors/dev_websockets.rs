use actix::prelude::*;
use actix_web_actors::ws;
use crate::actors::ws_server::{WsServer, Connect, Disconnect};

#[derive(Message)]
#[rtype(result = "()")]
pub struct ReloadMessage;

pub struct DevWebSocket {
    server_addr: Addr<WsServer>,
}

impl DevWebSocket {
    pub fn new(server_addr: Addr<WsServer>) -> Self {
        Self { server_addr }
    }
}

impl Actor for DevWebSocket {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let addr = ctx.address().recipient();
        self.server_addr.do_send(Connect { addr });
    }

    fn stopping(&mut self, ctx: &mut Self::Context) -> Running {
        let addr = ctx.address().recipient();
        self.server_addr.do_send(Disconnect { addr });
        Running::Stop
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for DevWebSocket {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Err(e) => log::error!("The live-reload connection failed: {:?}. Your browser might not auto-refresh when you save files. Try refreshing the page manually.", e),
            _ => (),
        }
    }
}

impl Handler<ReloadMessage> for DevWebSocket {
    type Result = ();

    fn handle(&mut self, _msg: ReloadMessage, ctx: &mut Self::Context) {
        ctx.text("reload");
    }
}