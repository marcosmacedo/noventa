use crate::actors::health::{GetSystemHealth, HealthActor};
use crate::actors::page_renderer::{HttpRequestInfo, RenderMessage};
use crate::actors::router::{MatchRoute, RouterActor};
use actix::{Addr, Recipient};
use actix_multipart::Multipart;
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
    payload: web::Payload,
    renderer: web::Data<Recipient<RenderMessage>>,
    template_path: String,
    path_params: HashMap<String, String>,
) -> HttpResponse {
    let (form_data, files) = if req.method() == actix_web::http::Method::POST {
        let content_type = req.headers().get("content-type").map(|v| v.to_str().unwrap_or("")).unwrap_or("");
        if content_type.starts_with("multipart/form-data") {
            let multipart = Multipart::new(req.headers(), payload);
            crate::fileupload::handle_multipart(multipart).await
        } else {
            let mut body = web::BytesMut::new();
            let mut stream = payload;
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.unwrap();
                body.extend_from_slice(&chunk);
            }
            let form_data = if let Ok(parsed) = serde_urlencoded::from_bytes::<HashMap<String, String>>(&body) {
                parsed.into_iter().map(|(k, v)| (k, serde_json::Value::String(v))).collect()
            } else {
                serde_json::Map::new()
            };
            (form_data, HashMap::new())
        }
    } else {
        (serde_json::Map::new(), HashMap::new())
    };

    let headers = req
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap().to_string()))
        .collect();

    let query_params: HashMap<String, String> =
        serde_urlencoded::from_str(req.query_string()).unwrap_or_default();

    let request_info = HttpRequestInfo {
        path: req.path().to_string(),
        method: req.method().to_string(),
        headers,
        form_data,
        files,
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
            if e.kind() == minijinja::ErrorKind::InvalidOperation && e.to_string() == "SHEDDING" {
                log::warn!("Shedding request due to high latency.");
                HttpResponse::ServiceUnavailable().body("Service Unavailable: Shedding load")
            } else {
                log::error!("Error rendering page: {}", e);
                HttpResponse::InternalServerError().finish()
            }
        }
        Err(e) => {
            log::error!("Mailbox error: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
pub async fn health_check(health_actor: web::Data<Addr<HealthActor>>) -> impl Responder {
    match health_actor.send(GetSystemHealth).await {
        Ok(health) => HttpResponse::Ok().json(health),
        Err(e) => {
            log::error!("Failed to get system health: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

pub async fn dynamic_route_handler(
    req: HttpRequest,
    payload: web::Payload,
    router: web::Data<Addr<RouterActor>>,
    renderer: web::Data<Recipient<RenderMessage>>,
) -> HttpResponse {
    let path = req.path().to_string();
    match router.send(MatchRoute(path)).await {
        Ok(Some((template_path, path_params))) => {
            handle_page(req, payload, renderer, template_path, path_params).await
        }
        Ok(None) => HttpResponse::NotFound().finish(),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}
