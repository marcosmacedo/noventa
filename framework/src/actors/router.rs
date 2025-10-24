use actix::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use crate::routing;

pub struct RouterActor {
    routes: Arc<RwLock<Vec<(String, PathBuf)>>>,
}

impl RouterActor {
    pub fn new() -> Self {
        let pages_dir = Path::new("./pages");
        let initial_routes = routing::get_routes(pages_dir);
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
        let new_routes = routing::get_routes(pages_dir);
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

        for (route_pattern, template_path) in routes.iter() {
            let mut params = HashMap::new();
            if let Some(captures) = path_to_regex(route_pattern).captures(&path) {
                for name in path_to_regex(route_pattern).capture_names().flatten() {
                    if let Some(value) = captures.name(name) {
                        params.insert(name.to_string(), value.as_str().to_string());
                    }
                }
                let mut template_path_str = template_path.to_str().unwrap().to_string();
                if template_path_str.starts_with("./") {
                    template_path_str = template_path_str[2..].to_string();
                }
                return Some((template_path_str, params));
            }
        }
        None
    }
}

fn path_to_regex(path: &str) -> regex::Regex {
    let pattern = path.split('/').map(|part| {
        if part.starts_with('{') && part.ends_with('}') {
            let param_name = &part[1..part.len() - 1];
            format!(r"(?P<{}>[^/]+)", param_name)
        } else {
            regex::escape(part)
        }
    }).collect::<Vec<_>>().join("/");
    regex::Regex::new(&format!("^{}$", pattern)).unwrap()
}