use crate::actors::health::{GetSystemHealth, HealthActor};
use crate::actors::page_renderer::{HttpRequestInfo, RenderMessage, RenderOutput};
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
use regex::Regex;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct CompiledRoute {
    pub regex: Regex,
    pub param_names: Vec<String>,
    pub template_path: PathBuf,
}

pub fn get_compiled_routes(pages_dir: &Path) -> Vec<CompiledRoute> {
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

    routes.sort_by(|(a, _), (b, _)| {
        let a_parts = a.split('/').count();
        let b_parts = b.split('/').count();
        let a_is_dynamic = a.contains('{');
        let b_is_dynamic = b.contains('{');

        b_parts.cmp(&a_parts).then(a_is_dynamic.cmp(&b_is_dynamic))
    });

    let mut final_routes = Vec::new();
    let mut registered_routes = HashMap::new();

    for (route_pattern, template_path) in routes {
        let route_key = route_pattern.split('{').next().unwrap_or("").to_string();
        if registered_routes.contains_key(&route_key) {
            panic!(
                "Route conflict detected: {}. A route with a similar path has already been registered.",
                route_pattern
            );
        }
        registered_routes.insert(route_key, route_pattern.contains('{'));

        log::debug!("Route registered: {} -> {}", route_pattern, template_path.display());
        final_routes.push(compile_route(route_pattern, template_path));
    }

    final_routes
}

fn compile_route(route_pattern: String, template_path: PathBuf) -> CompiledRoute {
    let mut param_names = Vec::new();
    
    let parts: Vec<String> = route_pattern
        .split('/')
        .skip(1) // Skip the initial empty string from the leading "/"
        .map(|part| {
            if part.starts_with('{') && part.ends_with('}') {
                let param_name = &part[1..part.len() - 1];
                let sanitized_name = param_name.replace('-', "_");
                param_names.push(sanitized_name.clone());
                format!(r"(?P<{}>[^/]+)", sanitized_name)
            } else {
                regex::escape(part)
            }
        })
        .collect();

    let regex_pattern = format!("^/{}$", parts.join("/"));

    let regex = Regex::new(&regex_pattern).unwrap_or_else(|e| {
        log::error!("Failed to compile regex for route: {}. Error: {}", route_pattern, e);
        Regex::new("$^").unwrap()
    });

    CompiledRoute {
        regex,
        param_names,
        template_path,
    }
}

#[deprecated(note = "Use get_compiled_routes instead")]
pub fn get_routes(_pages_dir: &Path) -> Vec<(String, PathBuf)> {
    vec![]
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
    _session: Option<&Session>,
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
    let request_info = build_http_request_info(&req, form_data, files, path_params, Some(&session));

    let session_manager = SessionManagerActor::new(session).start();

    let render_msg = RenderMessage {
        template_path,
        request_info: Arc::new(request_info),
        session_manager,
    };

    match renderer.send(render_msg).await {
        Ok(Ok(render_output)) => match render_output {
            RenderOutput::Html(html) => HttpResponse::Ok().content_type("text/html").body(html),
            RenderOutput::Redirect(url) => {
                if req.headers().contains_key("X-Requested-With") {
                    // It's an XHR request, send 200 OK with a custom header
                    HttpResponse::Ok()
                        .append_header(("X-Noventa-Redirect", url))
                        .finish()
                } else {
                    // It's a regular request, send a 303 redirect
                    HttpResponse::SeeOther()
                        .append_header(("Location", url))
                        .finish()
                }
            }
        },
        Ok(Err(mut detailed_error)) => {
            detailed_error.route = Some(req.path().to_string());
            if dev_mode {
                let html = crate::templates::render_structured_debug_error(&detailed_error);
                HttpResponse::Ok().content_type("text/html").body(html)
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

pub async fn handle_page_native(
    req: HttpRequest,
    payload: web::Payload,
    renderer: web::Data<Recipient<RenderMessage>>,
    session: Session,
    path_params: web::Path<HashMap<String, String>>,
    template_path: web::Data<String>,
) -> HttpResponse {
    let dev_mode = req.app_data::<web::Data<bool>>().map_or(false, |d| *d.get_ref());
    let full_template_path = template_path.get_ref().clone();
    let template_path_str = std::path::Path::new(&full_template_path).strip_prefix(&*crate::config::BASE_PATH).unwrap_or(std::path::Path::new(&full_template_path)).to_str().unwrap().to_string();
    let template_path_str = if template_path_str.starts_with("/") {
        template_path_str[1..].to_string()
    } else {
        template_path_str
    };
    handle_page(req, payload, renderer, session, template_path_str, path_params.into_inner(), dev_mode).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    use std::collections::HashMap;

    #[test]
    fn test_path_to_route() {
        let base_dir = Path::new("/tmp/pages");
        assert_eq!(path_to_route(Path::new("/tmp/pages/index.html"), base_dir), "/");
        assert_eq!(path_to_route(Path::new("/tmp/pages/about.html"), base_dir), "/about");
        assert_eq!(path_to_route(Path::new("/tmp/pages/blog/first-post.html"), base_dir), "/blog/first-post");
        assert_eq!(path_to_route(Path::new("/tmp/pages/users/[id].html"), base_dir), "/users/{id}");
        assert_eq!(path_to_route(Path::new("/tmp/pages/posts/[category]/[post_id].html"), base_dir), "/posts/{category}/{post-id}");
        assert_eq!(path_to_route(Path::new("/tmp/pages/a/[b]/c/[d].html"), base_dir), "/a/{b}/c/{d}");
        assert_eq!(path_to_route(Path::new("/tmp/pages/a-b_c.html"), base_dir), "/a-b-c");
    }

    #[test]
    fn test_get_compiled_routes() {
        let dir = tempdir().unwrap();
        let pages_dir = dir.path();

        fs::create_dir_all(pages_dir.join("blog")).unwrap();
        fs::File::create(pages_dir.join("index.html")).unwrap();
        fs::File::create(pages_dir.join("about.html")).unwrap();
        fs::File::create(pages_dir.join("blog/first-post.html")).unwrap();
        fs::create_dir_all(pages_dir.join("users")).unwrap();
        fs::File::create(pages_dir.join("users/[id].html")).unwrap();
        fs::create_dir_all(pages_dir.join("posts/[category]")).unwrap();
        fs::File::create(pages_dir.join("posts/[category]/[post-id].html")).unwrap();

        let routes = get_compiled_routes(pages_dir);

        assert_eq!(routes.len(), 5);

        // Test: /posts/{category}/{post-id}
        let post_route = routes.iter().find(|r| r.template_path.ends_with("posts/[category]/[post-id].html")).unwrap();
        assert!(post_route.regex.is_match("/posts/tech/123"));
        assert!(!post_route.regex.is_match("/posts/tech"));
        assert_eq!(post_route.param_names, vec!["category", "post_id"]);

        // Test: /users/{id}
        let user_route = routes.iter().find(|r| r.template_path.ends_with("users/[id].html")).unwrap();
        assert!(user_route.regex.is_match("/users/456"));
        assert!(!user_route.regex.is_match("/users/456/profile"));
        assert_eq!(user_route.param_names, vec!["id"]);

        // Test: /blog/first-post
        let blog_route = routes.iter().find(|r| r.template_path.ends_with("blog/first-post.html")).unwrap();
        assert!(blog_route.regex.is_match("/blog/first-post"));
        assert!(blog_route.param_names.is_empty());

        // Test: /about
        let about_route = routes.iter().find(|r| r.template_path.ends_with("about.html")).unwrap();
        assert!(about_route.regex.is_match("/about"));
        assert!(about_route.param_names.is_empty());

        // Test: /
        let index_route = routes.iter().find(|r| r.template_path.ends_with("index.html")).unwrap();
        assert!(index_route.regex.is_match("/"));
        assert!(index_route.param_names.is_empty());
    }

    #[test]
    #[should_panic(expected = "Route conflict detected")]
    fn test_get_routes_conflict() {
        let dir = tempdir().unwrap();
        let pages_dir = dir.path();

        fs::create_dir_all(pages_dir.join("conflict")).unwrap();
        fs::File::create(pages_dir.join("conflict.html")).unwrap();
        fs::File::create(pages_dir.join("conflict/index.html")).unwrap();

        get_compiled_routes(pages_dir);
    }

    #[test]
    fn test_routing_consistency_dev_vs_prod() {
        let dir = tempdir().unwrap();
        let pages_dir = dir.path();

        // Create test page structure
        fs::create_dir_all(pages_dir.join("blog")).unwrap();
        fs::File::create(pages_dir.join("index.html")).unwrap();
        fs::File::create(pages_dir.join("about.html")).unwrap();
        fs::File::create(pages_dir.join("blog/first-post.html")).unwrap();
        fs::create_dir_all(pages_dir.join("users")).unwrap();
        fs::File::create(pages_dir.join("users/[id].html")).unwrap();

        // Get compiled routes (prod mode)
        let compiled_routes = get_compiled_routes(pages_dir);

        // Test cases: (path, expected_template_relative_path, expected_params)
        let test_cases = vec![
            ("/", "index.html", HashMap::new()),
            ("/about", "about.html", HashMap::new()),
            ("/blog/first-post", "blog/first-post.html", HashMap::new()),
            ("/users/123", "users/[id].html", {
                let mut params = HashMap::new();
                params.insert("id".to_string(), "123".to_string());
                params
            }),
        ];

        for (path, expected_template, expected_params) in test_cases {
            // Test prod mode (compiled routes)
            let prod_result = compiled_routes.iter().find_map(|route| {
                if let Some(captures) = route.regex.captures(path) {
                    let params: HashMap<String, String> = route
                        .param_names
                        .iter()
                        .filter_map(|name| {
                            captures
                                .name(name)
                                .map(|value| (name.clone(), value.as_str().to_string()))
                        })
                        .collect();

                    // Get relative path from pages_dir
                    let template_path_str = if route.template_path.starts_with(pages_dir) {
                        route.template_path.strip_prefix(pages_dir).unwrap().to_str().unwrap().to_string()
                    } else {
                        route.template_path.to_str().unwrap().to_string()
                    };
                    Some((template_path_str, params))
                } else {
                    None
                }
            });

            // Test dev mode (simulated router logic)
            let dev_result = compiled_routes.iter().find_map(|route| {
                if let Some(captures) = route.regex.captures(path) {
                    let params: HashMap<String, String> = route
                        .param_names
                        .iter()
                        .filter_map(|name| {
                            captures
                                .name(name)
                                .map(|value| (name.clone(), value.as_str().to_string()))
                        })
                        .collect();

                    // Simulate what RouterActor does - strip BASE_PATH, but for test use pages_dir
                    let template_path_str = if route.template_path.starts_with(pages_dir) {
                        route.template_path.strip_prefix(pages_dir).unwrap().to_str().unwrap().to_string()
                    } else {
                        route.template_path.to_str().unwrap().to_string()
                    };
                    let template_path_str = if template_path_str.starts_with("/") {
                        template_path_str[1..].to_string()
                    } else {
                        template_path_str
                    };
                    Some((template_path_str, params))
                } else {
                    None
                }
            });

            // Assert both modes return the same result
            assert_eq!(prod_result, dev_result, "Routing mismatch for path: {}", path);

            if let Some((template_path, params)) = prod_result {
                assert_eq!(template_path, expected_template, "Template mismatch for path: {}", path);
                assert_eq!(params, expected_params, "Params mismatch for path: {}", path);
            }
        }
    }

    #[test]
    fn test_http_request_info_building() {
        use actix_web::test::TestRequest;
        use std::collections::HashMap;

        // Create a test request with various headers and query params
        let req = TestRequest::post()
            .uri("https://example.com/my/path?param1=value1&param2=value2")
            .insert_header(("user-agent", "TestBrowser/1.0"))
            .insert_header(("content-type", "application/x-www-form-urlencoded"))
            .insert_header(("content-length", "15"))
            .insert_header(("x-requested-with", "XMLHttpRequest"))
            .insert_header(("authorization", "Bearer test-token"))
            .insert_header(("cache-control", "no-cache"))
            .insert_header(("accept", "text/html,application/xhtml+xml"))
            .insert_header(("accept-language", "en-US,en;q=0.9"))
            .insert_header(("accept-encoding", "gzip, deflate"))
            .insert_header(("accept-charset", "utf-8"))
            .insert_header(("x-forwarded-for", "192.168.1.1, 10.0.0.1"))
            .insert_header(("referer", "https://example.com/previous"))
            .insert_header(("x-real-ip", "203.0.113.1"))
            .to_http_request();

        let form_data = {
            let mut map = serde_json::Map::new();
            map.insert("field1".to_string(), serde_json::Value::String("test_value".to_string()));
            map
        };

        let files = HashMap::new();
        let path_params = {
            let mut params = HashMap::new();
            params.insert("id".to_string(), "123".to_string());
            params.insert("category".to_string(), "electronics".to_string());
            params
        };

        // Create a minimal dummy session - since it's not used in the function
        // We'll just use a HashMap that implements the necessary traits
        let _dummy_session_data = HashMap::<String, String>::new();

        // Build HttpRequestInfo
        let request_info = build_http_request_info(&req, form_data.clone(), files.clone(), path_params.clone(), None);

        // Verify core request information
        assert_eq!(request_info.path, "/my/path");
        assert_eq!(request_info.method, "POST");
        assert_eq!(request_info.scheme, "https");
        assert_eq!(request_info.host, "example.com");
        assert_eq!(request_info.is_secure, true);
        assert_eq!(request_info.is_xhr, true);

        // Verify URL construction
        assert_eq!(request_info.url, "https://example.com/my/path?param1=value1&param2=value2");
        assert_eq!(request_info.base_url, "https://example.com/my/path");
        assert_eq!(request_info.host_url, "https://example.com");
        assert_eq!(request_info.url_root, "https://example.com");
        assert_eq!(request_info.full_path, "/my/path?param1=value1&param2=value2");

        // Verify headers are captured
        assert_eq!(request_info.user_agent, Some("TestBrowser/1.0".to_string()));
        assert_eq!(request_info.content_type, Some("application/x-www-form-urlencoded".to_string()));
        assert_eq!(request_info.content_length, Some(15));
        assert_eq!(request_info.authorization, Some("Bearer test-token".to_string()));
        assert_eq!(request_info.cache_control, Some("no-cache".to_string()));
        assert_eq!(request_info.referrer, Some("https://example.com/previous".to_string()));

        // Verify query parameters
        assert_eq!(request_info.query_params.get("param1"), Some(&"value1".to_string()));
        assert_eq!(request_info.query_params.get("param2"), Some(&"value2".to_string()));

        // Verify path parameters
        assert_eq!(request_info.path_params.get("id"), Some(&"123".to_string()));
        assert_eq!(request_info.path_params.get("category"), Some(&"electronics".to_string()));

        // Verify form data
        assert_eq!(request_info.form_data, form_data);

        // Verify multi-value headers
        assert!(request_info.accept_mimetypes.contains(&"text/html".to_string()));
        assert!(request_info.accept_mimetypes.contains(&"application/xhtml+xml".to_string()));
        assert!(request_info.accept_languages.contains(&"en-US".to_string()));
        assert!(request_info.accept_encodings.contains(&"gzip".to_string()));
        assert!(request_info.accept_charsets.contains(&"utf-8".to_string()));

        // Verify forwarded headers
        assert!(request_info.access_route.contains(&"192.168.1.1".to_string()));
        assert!(request_info.access_route.contains(&"10.0.0.1".to_string()));

        // Verify remote address extraction
        assert_eq!(request_info.remote_addr, Some("192.168.1.1".to_string()));
    }

    #[test]
    fn test_compile_route() {
        use std::path::PathBuf;

        // Test simple route without parameters
        let route = compile_route("/about".to_string(), PathBuf::from("about.html"));
        assert_eq!(route.param_names.len(), 0);
        assert!(route.regex.is_match("/about"));
        assert!(!route.regex.is_match("/contact"));

        // Test route with single parameter
        let route = compile_route("/users/{id}".to_string(), PathBuf::from("users/[id].html"));
        assert_eq!(route.param_names, vec!["id"]);
        assert!(route.regex.is_match("/users/123"));
        assert!(!route.regex.is_match("/users/"));
        assert!(!route.regex.is_match("/users/123/456"));

        // Test route with multiple parameters
        let route = compile_route("/blog/{year}/{month}/{slug}".to_string(), PathBuf::from("blog/[year]/[month]/[slug].html"));
        assert_eq!(route.param_names, vec!["year", "month", "slug"]);
        assert!(route.regex.is_match("/blog/2023/12/my-post"));
        assert!(!route.regex.is_match("/blog/2023/12"));

        // Test parameter name sanitization (dashes to underscores)
        let route = compile_route("/posts/{post-id}".to_string(), PathBuf::from("posts/[post-id].html"));
        assert_eq!(route.param_names, vec!["post_id"]);
        assert!(route.regex.is_match("/posts/abc-123"));
    }

    #[actix_rt::test]
    async fn test_parse_request_body() {
        // TODO: Add test when payload handling is simplified
    }

    #[actix_rt::test]
    async fn test_handle_page() {
        // TODO: Add test when session handling is simplified
    }

    #[actix_rt::test]
    async fn test_dynamic_route_handler() {
        // TODO: Add test when session handling is simplified
    }
}

