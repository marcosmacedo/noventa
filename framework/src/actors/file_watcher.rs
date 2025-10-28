use actix::prelude::*;
use notify::{RecommendedWatcher, Watcher, RecursiveMode, Result};
use ignore::gitignore::Gitignore;
use std::fs;
use std::path::Path;
use crate::actors::ws_server::{WsServer, BroadcastReload};
use crate::actors::router::{RouterActor, ReloadRoutes};
use crate::actors::template_renderer::{TemplateRendererActor, UpdateComponents};
use crate::actors::interpreter::{PythonInterpreterActor, ReloadInterpreter};

pub struct FileWatcherActor {
    ws_server_addr: Addr<WsServer>,
    router_addr: Addr<RouterActor>,
    template_renderer_addr: Addr<TemplateRendererActor>,
    interpreter_addr: Addr<PythonInterpreterActor>,
    watcher: Option<RecommendedWatcher>,
    components_path: std::path::PathBuf,
    pages_path: std::path::PathBuf,
    layouts_path: std::path::PathBuf,
}

impl FileWatcherActor {
    pub fn new(ws_server_addr: Addr<WsServer>, router_addr: Addr<RouterActor>, template_renderer_addr: Addr<TemplateRendererActor>, interpreter_addr: Addr<PythonInterpreterActor>) -> Self {
        Self {
            ws_server_addr,
            router_addr,
            template_renderer_addr,
            interpreter_addr,
            watcher: None,
            components_path: std::path::PathBuf::new(),
            pages_path: std::path::PathBuf::new(),
            layouts_path: std::path::PathBuf::new(),
        }
    }
}

impl Actor for FileWatcherActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        log::debug!("File watcher is up and running!");

        let ws_server_addr = self.ws_server_addr.clone();
        let router_addr = self.router_addr.clone();
        let template_renderer_addr = self.template_renderer_addr.clone();
        let interpreter_addr = self.interpreter_addr.clone();

        let components_path = std::path::PathBuf::from("./components");
        let pages_path = fs::canonicalize("./pages").unwrap();
        let layouts_path = fs::canonicalize("./layouts").unwrap();

        self.components_path = components_path.clone();
        self.pages_path = pages_path.clone();
        self.layouts_path = layouts_path.clone();

        let (gitignore, _) = ignore::gitignore::Gitignore::new("./.gitignore");
        let current_dir = std::env::current_dir().unwrap();

        // Create the watcher first
        let mut watcher = match notify::recommended_watcher(move |res: Result<notify::Event>| {
            match res {
                Ok(event) => {
                    if let Some(path) = event.paths.first() {
                        let relative_path = path.strip_prefix(&current_dir).unwrap_or(path);
                        if gitignore.matched(relative_path, false).is_ignore() {
                            return;
                        }

                        log::debug!("Detected a change in: {:?}", path);

                        if path.extension().map_or(false, |ext| ext == "py") {
                            log::debug!("A Python file has changed. Reloading the interpreter now!");
                            interpreter_addr.do_send(ReloadInterpreter);
                        }

                        if path.starts_with(&pages_path) {
                            log::debug!("A page has changed. Reloading the routes now!");
                            router_addr.do_send(ReloadRoutes);
                        } else if path.starts_with(&layouts_path) {
                            // We don't need to do anything here, but we want to avoid the component scan
                        } else if path.starts_with(&components_path) {
                            log::debug!("A component has changed. Rescanning all components now!");
                            match crate::components::scan_components(&components_path) {
                                Ok(components) => {
                                    template_renderer_addr.do_send(UpdateComponents(components));
                                }
                                Err(e) => {
                                    log::error!("Failed to rescan components: {}", e);
                                }
                            }
                        }
                    }
                    ws_server_addr.do_send(BroadcastReload);
                }
                Err(e) => log::error!("Oh no, a file watch error occurred: {:?}", e),
            }
        }) {
            Ok(watcher) => watcher,
            Err(e) => {
                log::error!("We couldn't create the file watcher: {:?}. Live reloading will be disabled.", e);
                // Stop the actor if the watcher cannot be created.
                _ctx.stop();
                return;
            }
        };

        // Watch the current directory recursively.
        if let Err(e) = watcher.watch(Path::new("."), RecursiveMode::Recursive) {
            log::error!("We couldn't watch the current directory: {:?}", e);
        }

        // Important: keep the watcher alive for the actor’s lifetime
        self.watcher = Some(watcher);
        log::trace!("Watcher stored in actor: {:?}", self.watcher.is_some());
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        log::debug!("File watcher is shutting down. Goodbye!");
        Running::Stop
    }
}
