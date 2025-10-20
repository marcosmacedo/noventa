use actix::prelude::*;
use notify::{RecommendedWatcher, Watcher, RecursiveMode, Result};
use std::fs;
use crate::actors::ws_server::{WsServer, BroadcastReload};
use crate::actors::router::{RouterActor, ReloadRoutes};

pub struct FileWatcherActor {
    ws_server_addr: Addr<WsServer>,
    router_addr: Addr<RouterActor>,
    watcher: Option<RecommendedWatcher>,
}

impl FileWatcherActor {
    pub fn new(ws_server_addr: Addr<WsServer>, router_addr: Addr<RouterActor>) -> Self {
        Self {
            ws_server_addr,
            router_addr,
            watcher: None,
        }
    }
}

impl Actor for FileWatcherActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        log::info!("FileWatcherActor started");

        let ws_server_addr = self.ws_server_addr.clone();
        let router_addr = self.router_addr.clone();

        // Create the watcher first
        let mut watcher = match notify::recommended_watcher(move |res: Result<notify::Event>| {
            match res {
                Ok(event) => {
                    if let Some(path) = event.paths.first() {
                        log::debug!("File changed: {:?}", path);
                    }
                    ws_server_addr.do_send(BroadcastReload);
                    router_addr.do_send(ReloadRoutes);
                }
                Err(e) => log::error!("Watch error: {:?}", e),
            }
        }) {
            Ok(watcher) => watcher,
            Err(e) => {
                log::error!("Failed to create file watcher: {:?}", e);
                // Stop the actor if the watcher cannot be created.
                _ctx.stop();
                return;
            }
        };

        let paths_to_watch = ["./components", "./pages", "./layouts"];

        for path_str in paths_to_watch.iter() {
            match fs::canonicalize(path_str) {
                Ok(path) => {
                    log::info!("Watching path: {:?}", path);
                    if let Err(e) = watcher.watch(&path, RecursiveMode::Recursive) {
                        log::error!("Failed to watch path {:?}: {:?}", path, e);
                    }
                }
                Err(e) => {
                    log::warn!("Skipping path {:?}: {:?}", path_str, e);
                }
            }
        }

        // Important: keep the watcher alive for the actor’s lifetime
        self.watcher = Some(watcher);
        log::info!("Watcher stored in actor: {:?}", self.watcher.is_some());
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        log::info!("File watcher is stopping");
        Running::Stop
    }
}
