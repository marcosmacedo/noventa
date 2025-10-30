use actix::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use crate::routing::{self, CompiledRoute};

pub struct RouterActor {
    routes: Arc<RwLock<Vec<CompiledRoute>>>,
}

impl RouterActor {
    pub fn new() -> Self {
        let pages_dir = Path::new("./pages");
        let initial_routes = routing::get_compiled_routes(pages_dir);
        Self {
            routes: Arc::new(RwLock::new(initial_routes)),
        }
    }
}

impl Actor for RouterActor {
    type Context = Context<Self>;
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct ReloadRoutes;

impl Handler<ReloadRoutes> for RouterActor {
    type Result = ();

    fn handle(&mut self, _msg: ReloadRoutes, _ctx: &mut Context<Self>) {
        log::debug!("A file change was detected. We're reloading the routes now!");
        let pages_dir = Path::new("./pages");
        let new_routes = routing::get_compiled_routes(pages_dir);
        let mut routes = self.routes.write().unwrap();
        *routes = new_routes;
        log::debug!("Routes have been successfully reloaded.");
    }
}

#[derive(Message)]
#[rtype(result = "Option<(String, HashMap<String, String>)>")]
pub struct MatchRoute(pub String);

impl Handler<MatchRoute> for RouterActor {
    type Result = Option<(String, HashMap<String, String>)>;

    fn handle(&mut self, msg: MatchRoute, _ctx: &mut Context<Self>) -> Self::Result {
        let routes = self.routes.read().unwrap();
        let path = msg.0;

        for route in routes.iter() {
            if let Some(captures) = route.regex.captures(&path) {
                let params: HashMap<String, String> = route
                    .param_names
                    .iter()
                    .filter_map(|name| {
                        captures
                            .name(name)
                            .map(|value| (name.clone(), value.as_str().to_string()))
                    })
                    .collect();

                let mut template_path_str = route.template_path.to_str().unwrap().to_string();
                if template_path_str.starts_with("./") {
                    template_path_str = template_path_str[2..].to_string();
                }
                return Some((template_path_str, params));
            }
        }
        None
    }
}