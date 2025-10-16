use crate::actors::orchestrator::{HandleRequest, HttpOrchestratorActor};
use crate::components::Component;
use actix::Addr;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn get_routes(pages_dir: &Path) -> Vec<(String, PathBuf)> {
    let mut routes = Vec::new();
    for entry in WalkDir::new(pages_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("html") {
            let route = path_to_route(path, pages_dir);
            routes.push((route, path.to_path_buf()));
        }
    }

    // Sort routes to handle conflicts (more specific routes first)
    routes.sort_by(|(a, _), (b, _)| {
        let a_parts = a.split('/').count();
        let b_parts = b.split('/').count();
        let a_is_dynamic = a.contains('{');
        let b_is_dynamic = b.contains('{');

        b_parts.cmp(&a_parts)
            .then(a_is_dynamic.cmp(&b_is_dynamic))
    });

    let mut registered_routes = HashMap::new();
    let mut final_routes = Vec::new();

    for (route, template_path) in routes {
        let route_key = route.split('{').next().unwrap_or("").to_string();
        if registered_routes.contains_key(&route_key) {
            let is_dynamic = route.contains('{');
            let existing_is_dynamic = registered_routes[&route_key];
            if is_dynamic != existing_is_dynamic {
                log::error!("Route conflict detected: {}", route);
                continue;
            }
        }

        registered_routes.insert(route_key, route.contains('{'));
        log::info!("Registering route: {} -> {:?}", route, template_path);
        final_routes.push((route, template_path));
    }

    final_routes
}

fn path_to_route(path: &Path, base_dir: &Path) -> String {
    let relative_path = path.strip_prefix(base_dir).unwrap();
    let mut route = String::from("/");
    for component in relative_path.components() {
        let segment = component.as_os_str().to_str().unwrap();
        if segment.ends_with(".html") {
            let stem = &segment[..segment.len() - 5];
            if stem != "index" {
                route.push_str(stem);
            }
        } else {
            route.push_str(segment);
            route.push('/');
        }
    }

    route = route
        .replace("[", "{")
        .replace("]", "}");

    if route.len() > 1 && route.ends_with('/') {
        route.pop();
    }

    route
}

pub async fn handle_page(
    _req: HttpRequest,
    components: web::Data<HashMap<String, Component>>,
    orchestrator: web::Data<Addr<HttpOrchestratorActor>>,
    template_name: String,
) -> impl Responder {
    let component = components.get("hello").unwrap();
    let res = orchestrator
        .send(HandleRequest {
            component_name: component.id.clone(),
            template_name,
        })
        .await;

    match res {
        Ok(Ok(rendered_page)) => HttpResponse::Ok().body(rendered_page),
        _ => HttpResponse::InternalServerError().finish(),
    }
}