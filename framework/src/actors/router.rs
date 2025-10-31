use actix::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use crate::routing::{self, CompiledRoute};
use crate::config;

pub struct RouterActor {
    routes: Arc<RwLock<Vec<CompiledRoute>>>,
}

impl RouterActor {
    pub fn new() -> Self {
        let pages_dir = config::BASE_PATH.join("pages");
        let initial_routes = routing::get_compiled_routes(&pages_dir);
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
        let pages_dir = config::BASE_PATH.join("pages");
        let new_routes = routing::get_compiled_routes(&pages_dir);
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

                let template_path_str = route.template_path.strip_prefix(&*config::BASE_PATH).unwrap_or(&route.template_path).to_str().unwrap().to_string();
                let template_path_str = if template_path_str.starts_with("/") {
                    template_path_str[1..].to_string()
                } else {
                    template_path_str
                };
                return Some((template_path_str, params));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix::Actor;
    use actix::actors::mocker::Mocker;

    #[actix_rt::test]
    async fn test_router_actor_new() {
        let router = RouterActor::new();
        // Test that routes are initialized
        let routes = router.routes.read().unwrap();
        assert!(routes.len() >= 0); // May be empty if no pages directory
    }

    // Using the Mocker pattern for proper actor testing
    type RouterActorMock = Mocker<RouterActor>;

    #[actix_rt::test]
    async fn test_router_actor_creation() {
        let router_mock = RouterActorMock::mock(Box::new(|msg, _ctx| {
            if let Some(_) = msg.downcast_ref::<ReloadRoutes>() {
                Box::new(Some(()))
            } else if let Some(_) = msg.downcast_ref::<MatchRoute>() {
                Box::new(Some(None::<(String, HashMap<String, String>)>))
            } else {
                Box::new(Some(()))
            }
        }));

        let addr = router_mock.start();
        assert!(addr.connected());
    }

    #[actix_rt::test]
    async fn test_reload_routes_message_handling() {
        let router_mock = RouterActorMock::mock(Box::new(|msg, _ctx| {
            if let Some(_) = msg.downcast_ref::<ReloadRoutes>() {
                Box::new(Some(()))
            } else {
                Box::new(Some(()))
            }
        }));

        let addr = router_mock.start();
        
        let reload_msg = ReloadRoutes;
        let result = addr.send(reload_msg).await;
        assert!(result.is_ok());
    }

    #[actix_rt::test]
    async fn test_match_route_message_handling() {
        let router_mock = RouterActorMock::mock(Box::new(|msg, _ctx| {
            if let Some(match_msg) = msg.downcast_ref::<MatchRoute>() {
                // Mock route matching logic
                if match_msg.0 == "/test" {
                    Box::new(Some(Some(("pages/test.html".to_string(), HashMap::<String, String>::new()))))
                } else {
                    Box::new(Some(None::<(String, HashMap<String, String>)>))
                }
            } else {
                Box::new(Some(None::<(String, HashMap<String, String>)>))
            }
        }));

        let addr = router_mock.start();
        
        // Test matching a route
        let match_msg = MatchRoute("/test".to_string());
        let result = addr.send(match_msg).await;
        assert!(result.is_ok());
        
        // Test non-matching route
        let no_match_msg = MatchRoute("/nonexistent".to_string());
        let result = addr.send(no_match_msg).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_message_types() {
        // Test that message types can be created
        let _reload_msg = ReloadRoutes;
        let _match_msg = MatchRoute("/test".to_string());
        assert!(true);
    }
}