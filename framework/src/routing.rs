use crate::actors::page_renderer::{HttpRequestInfo, PageRendererActor, RenderMessage};
use actix::Addr;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use futures_util::stream::StreamExt;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use walkdir::WalkDir;

pub fn get_routes(pages_dir: &Path) -> Vec<(String, PathBuf)> {
    let mut routes: Vec<(String, PathBuf)> = WalkDir::new(pages_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file() && e.path().extension().and_then(|s| s.to_str()) == Some("html"))
        .map(|e| {
            let path = e.path().to_path_buf();
            let route = path_to_route(&path, pages_dir);
            (route, path)
        })
        .collect();

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
        let is_dynamic = route.contains('{');

        match registered_routes.entry(route_key) {
            std::collections::hash_map::Entry::Occupied(entry) => {
                if *entry.get() != is_dynamic {
                    log::error!("Route conflict detected: {}", route);
                    continue;
                }
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(is_dynamic);
            }
        }

        log::info!("Registering route: {} -> {:?}", route, template_path);
        final_routes.push((route, template_path));
    }

    final_routes
}

fn path_to_route(path: &Path, base_dir: &Path) -> String {
    let relative_path = match path.strip_prefix(base_dir) {
        Ok(p) => p,
        Err(_) => return String::new(),
    };

    let route_parts: Vec<String> = relative_path
        .components()
        .map(|comp| comp.as_os_str().to_string_lossy().into_owned())
        .filter_map(|segment| {
            if segment.ends_with(".html") {
                let stem = segment.strip_suffix(".html").unwrap();
                if stem != "index" {
                    Some(stem.to_string())
                } else {
                    None
                }
            } else {
                Some(segment)
            }
        })
        .collect();

    let mut route = format!("/{}", route_parts.join("/"));
    route = route.replace('[', "{").replace(']', "}");

    if route.len() > 1 && route.ends_with('/') {
        route.pop();
    }

    if route.is_empty() {
        "/".to_string()
    } else {
        route
    }
}

pub async fn handle_page(
    req: HttpRequest,
    mut payload: web::Payload,
    renderer: web::Data<Addr<PageRendererActor>>,
    template_path: String,
) -> impl Responder {
    let mut form_data = serde_json::Map::new();

    if req.method() == actix_web::http::Method::POST {
        let mut body = web::BytesMut::new();
        while let Some(chunk) = payload.next().await {
            let chunk = chunk.unwrap();
            body.extend_from_slice(&chunk);
        }
        if let Ok(parsed) = serde_urlencoded::from_bytes::<HashMap<String, String>>(&body) {
            for (key, value) in parsed {
                form_data.insert(key, serde_json::Value::String(value));
            }
        }
    }

    let query_params: HashMap<String, String> =
        serde_urlencoded::from_str(req.query_string()).unwrap_or_default();
    let path_params: HashMap<String, String> = req
        .match_info()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let request_info = HttpRequestInfo {
        path: req.path().to_string(),
        method: req.method().to_string(),
        form_data,
        query_params,
        path_params,
    };

    let render_msg = RenderMessage {
        template_path,
        request_info: Arc::new(request_info),
    };

    match renderer.send(render_msg).await {
        Ok(Ok(rendered)) => HttpResponse::Ok().body(rendered),
        Ok(Err(e)) => {
            log::error!("Error rendering page: {}", e);
            HttpResponse::InternalServerError().finish()
        }
        Err(e) => {
            log::error!("Mailbox error: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}