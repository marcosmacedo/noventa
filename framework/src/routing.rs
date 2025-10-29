use crate::actors::health::{GetSystemHealth, HealthActor};
use crate::actors::page_renderer::{HttpRequestInfo, RenderMessage};
use crate::actors::router::{MatchRoute, RouterActor};
use crate::actors::session_manager::SessionManagerActor;
use actix::{Actor, Addr, Recipient};
use actix_multipart::Multipart;
use actix_session::Session;
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
            .then(b_is_dynamic.cmp(&a_is_dynamic)) // Prioritize dynamic routes
    });

    let mut registered_routes = HashMap::new();
    let mut final_routes = Vec::new();

    for (route, template_path) in routes {
        let route_key = route.split('{').next().unwrap_or("").to_string();
        let is_dynamic = route.contains('{');

        if registered_routes.contains_key(&route_key) {
            panic!("Route conflict detected: {}. A route with a similar path has already been registered.", route);
        }
        registered_routes.insert(route_key, is_dynamic);

        log::debug!("Route registered: {} -> {}", route, template_path.display());
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
                    Some(stem.replace('_', "-"))
                } else {
                    None
                }
            } else {
                Some(segment.replace('_', "-"))
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

async fn parse_request_body(
    req: &HttpRequest,
    mut payload: web::Payload,
) -> (serde_json::Map<String, serde_json::Value>, HashMap<String, crate::actors::page_renderer::FilePart>) {
    if req.method() == actix_web::http::Method::POST {
        let content_type = req.headers().get("content-type").map(|v| v.to_str().unwrap_or("")).unwrap_or("");
        if content_type.starts_with("multipart/form-data") {
            let multipart = Multipart::new(req.headers(), payload);
            crate::fileupload::handle_multipart(multipart).await
        } else {
            let mut body = web::BytesMut::new();
            while let Some(chunk) = payload.next().await {
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
    }
}

fn build_http_request_info(
    req: &HttpRequest,
    form_data: serde_json::Map<String, serde_json::Value>,
    files: HashMap<String, crate::actors::page_renderer::FilePart>,
    path_params: HashMap<String, String>,
    _session: &Session,
) -> HttpRequestInfo {
    let headers = req
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap().to_string()))
        .collect();

    let query_params: HashMap<String, String> =
        serde_urlencoded::from_str(req.query_string()).unwrap_or_default();

    let scheme = req.connection_info().scheme().to_string();
    let host = req.connection_info().host().to_string();
    let remote_addr = req.connection_info().realip_remote_addr().map(|s| s.to_string());
    let full_path = if req.query_string().is_empty() {
        req.path().to_string()
    } else {
        format!("{}?{}", req.path(), req.query_string())
    };
    let url = format!("{}://{}{}", scheme, host, full_path);
    let base_url = format!("{}://{}{}", scheme, host, req.path());
    let host_url = format!("{}://{}", scheme, host);
    let url_root = format!("{}://{}", scheme, host);
    let query_string = req.query_string().as_bytes().to_vec();
    let cookies = req.cookies()
        .map(|c| c.iter().map(|c| (c.name().to_string(), c.value().to_string())).collect())
        .unwrap_or_default();
    let user_agent = req.headers().get("user-agent").and_then(|v| v.to_str().ok()).map(|s| s.to_string());
    let content_type = req.headers().get("content-type").and_then(|v| v.to_str().ok()).map(|s| s.to_string());
    let content_length = req.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|s| s.parse::<usize>().ok());
    let is_secure = scheme == "https";
    let is_xhr = req.headers().get("x-requested-with").map_or(false, |v| v == "XMLHttpRequest");

    let get_header_values = |key: &str| -> Vec<String> {
        req.headers()
            .get_all(key)
            .flat_map(|v| v.to_str().unwrap_or("").split(','))
            .map(|s| s.trim().to_string())
            .collect()
    };

    let get_header_value = |key: &str| -> Option<String> {
        req.headers().get(key).and_then(|v| v.to_str().ok()).map(|s| s.to_string())
    };

    HttpRequestInfo {
        path: req.path().to_string(),
        method: req.method().to_string(),
        headers,
        form_data,
        files,
        query_params,
        path_params,
        scheme,
        host,
        remote_addr,
        url,
        base_url,
        host_url,
        url_root,
        full_path,
        query_string,
        cookies,
        user_agent,
        content_type,
        content_length,
        is_secure,
        is_xhr,
        accept_charsets: get_header_values("accept-charset"),
        accept_encodings: get_header_values("accept-encoding"),
        accept_languages: get_header_values("accept-language"),
        accept_mimetypes: get_header_values("accept"),
        access_route: get_header_values("x-forwarded-for"),
        authorization: get_header_value("authorization"),
        cache_control: get_header_value("cache-control"),
        content_encoding: get_header_value("content-encoding"),
        content_md5: get_header_value("content-md5"),
        date: get_header_value("date"),
        if_match: get_header_values("if-match"),
        if_modified_since: get_header_value("if-modified-since"),
        if_none_match: get_header_values("if-none-match"),
        if_range: get_header_value("if-range"),
        if_unmodified_since: get_header_value("if-unmodified-since"),
        max_forwards: get_header_value("max-forwards"),
        pragma: get_header_value("pragma"),
        range: get_header_value("range"),
        referrer: get_header_value("referer"),
        remote_user: get_header_value("remote-user"),
    }
}

pub async fn handle_page(
    req: HttpRequest,
    payload: web::Payload,
    renderer: web::Data<Recipient<RenderMessage>>,
    session: Session,
    template_path: String,
    path_params: HashMap<String, String>,
    dev_mode: bool,
) -> HttpResponse {
    let (form_data, files) = parse_request_body(&req, payload).await;
    let request_info = build_http_request_info(&req, form_data, files, path_params, &session);

    let session_manager = SessionManagerActor::new(session).start();

    let render_msg = RenderMessage {
        template_path,
        request_info: Arc::new(request_info),
        session_manager,
    };

    match renderer.send(render_msg).await {
        Ok(Ok(rendered)) => HttpResponse::Ok().body(rendered),
        Ok(Err(mut detailed_error)) => {
            detailed_error.route = Some(req.path().to_string());
            if dev_mode {
                let html = crate::templates::render_structured_debug_error(&detailed_error);
                HttpResponse::InternalServerError().content_type("text/html").body(html)
            } else {
                let html = crate::templates::render_production_error(&detailed_error);
                HttpResponse::InternalServerError().content_type("text/html").body(html)
            }
        }
        Err(e) => {
            log::error!("A mailbox error occurred: {}. This might indicate a problem with the server's internal communication.", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
pub async fn health_check(health_actor: web::Data<Addr<HealthActor>>) -> impl Responder {
    match health_actor.send(GetSystemHealth).await {
        Ok(health) => HttpResponse::Ok().json(health),
        Err(e) => {
            log::error!("Could not retrieve system health: {}. The health check actor might be experiencing issues.", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

pub async fn dynamic_route_handler(
    req: HttpRequest,
    payload: web::Payload,
    router: web::Data<Addr<RouterActor>>,
    renderer: web::Data<Recipient<RenderMessage>>,
    session: Session,
) -> HttpResponse {
    let path = req.path().to_string();
    match router.send(MatchRoute(path)).await {
        Ok(Some((template_path, path_params))) => {
            let dev_mode = req.app_data::<web::Data<bool>>().map_or(false, |d| *d.get_ref());
            handle_page(req, payload, renderer, session, template_path, path_params, dev_mode).await
        }
        Ok(None) => {
            let dev_mode = req.app_data::<web::Data<bool>>().map_or(false, |d| *d.get_ref());
            if dev_mode && req.path() == "/" {
                // In dev mode, if no / page is found, show a welcome page
                const DEV_MODE_INDEX: &str = include_str!("templates/dev_mode_index.html");
                HttpResponse::Ok().content_type("text/html").body(DEV_MODE_INDEX)
            } else {
                HttpResponse::NotFound().finish()
            }
        }
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_path_to_route() {
        let base_dir = Path::new("/tmp/pages");

        // Test case 1: Index file
        let path1 = Path::new("/tmp/pages/index.html");
        assert_eq!(path_to_route(path1, base_dir), "/");

        // Test case 2: Simple route
        let path2 = Path::new("/tmp/pages/about.html");
        assert_eq!(path_to_route(path2, base_dir), "/about");

        // Test case 3: Nested route
        let path3 = Path::new("/tmp/pages/blog/first-post.html");
        assert_eq!(path_to_route(path3, base_dir), "/blog/first-post");

        // Test case 4: Dynamic route
        let path4 = Path::new("/tmp/pages/users/[id].html");
        assert_eq!(path_to_route(path4, base_dir), "/users/{id}");
        
        // Test case 5: Nested dynamic route
        let path5 = Path::new("/tmp/pages/posts/[category]/[post_id].html");
        assert_eq!(path_to_route(path5, base_dir), "/posts/{category}/{post-id}");
        // Test case 6: Multiple dynamic segments
        let path6 = Path::new("/tmp/pages/a/[b]/c/[d].html");
        assert_eq!(path_to_route(path6, base_dir), "/a/{b}/c/{d}");

        // Test case 7: Path with special characters
        let path7 = Path::new("/tmp/pages/a-b_c.html");
        assert_eq!(path_to_route(path7, base_dir), "/a-b-c");

        // Test case 8: Empty path
        let path8 = Path::new("/tmp/pages/.html");
        assert_eq!(path_to_route(path8, base_dir), "/");
    }
    #[test]
    #[should_panic(expected = "Route conflict detected")]
    fn test_get_routes_conflict() {
        let dir = tempdir().unwrap();
        let pages_dir = dir.path();

        // Create dummy files and directories that will cause a conflict
        fs::create_dir_all(pages_dir.join("conflict")).unwrap();
        fs::File::create(pages_dir.join("conflict.html")).unwrap();
        fs::File::create(pages_dir.join("conflict/index.html")).unwrap();

        // This should panic
        get_routes(pages_dir);
    }
}

    #[test]
    fn test_get_routes() {
        use std::fs;
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let pages_dir = dir.path();

        // Create dummy files and directories
        fs::create_dir_all(pages_dir.join("blog")).unwrap();
        fs::File::create(pages_dir.join("index.html")).unwrap();
        fs::File::create(pages_dir.join("about.html")).unwrap();
        fs::File::create(pages_dir.join("blog/index.html")).unwrap();
        fs::File::create(pages_dir.join("blog/first-post.html")).unwrap();
        fs::create_dir_all(pages_dir.join("users/[id]")).unwrap();
        fs::File::create(pages_dir.join("users/[id]/profile.html")).unwrap();

        let routes = get_routes(pages_dir);

        let expected_routes: Vec<(String, PathBuf)> = vec![
            ("/users/{id}/profile".to_string(), pages_dir.join("users/[id]/profile.html")),
            ("/blog/first-post".to_string(), pages_dir.join("blog/first-post.html")),
            ("/about".to_string(), pages_dir.join("about.html")),
            ("/blog".to_string(), pages_dir.join("blog/index.html")),
            ("/".to_string(), pages_dir.join("index.html")),
        ]
        .into_iter()
        .map(|(r, p)| (r, p.canonicalize().unwrap()))
        .collect();

        let mut actual_routes: Vec<(String, PathBuf)> = routes
            .into_iter()
            .map(|(r, p)| (r, p.canonicalize().unwrap()))
            .collect();

        actual_routes.sort_by(|(a, _), (b, _)| a.cmp(b));
        let mut expected_routes_sorted = expected_routes;
        expected_routes_sorted.sort_by(|(a, _), (b, _)| a.cmp(b));

        assert_eq!(actual_routes, expected_routes_sorted);
    }

