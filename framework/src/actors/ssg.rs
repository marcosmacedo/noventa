use actix::prelude::*;
use std::path::{Path, PathBuf};
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io;
use crate::config;
use crate::routing;
use crate::static_assets;

#[derive(Message)]
#[rtype(result = "io::Result<()>")]
pub struct SsgMessage {
    pub output_path: PathBuf,
}

pub struct SSGActor;

impl SSGActor {
    pub fn new() -> Self {
        SSGActor
    }
}

impl Actor for SSGActor {
    type Context = Context<Self>;
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

impl Handler<SsgMessage> for SSGActor {
    type Result = ResponseFuture<io::Result<()>>;

    fn handle(&mut self, msg: SsgMessage, _ctx: &mut Context<Self>) -> Self::Result {
        Box::pin(async move {
            log::info!("Static site generation started. Output path: {:?}", msg.output_path);

            if msg.output_path.exists() {
                fs::remove_dir_all(&msg.output_path)?;
            }
            fs::create_dir_all(&msg.output_path)?;

            let client = reqwest::Client::builder()
                .danger_accept_invalid_certs(true)
                .build()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            let address = crate::config::CONFIG.server_address.as_deref().unwrap_or("127.0.0.1");
            let port = crate::config::CONFIG.port.unwrap_or(8080);
            let base_url = format!("http://{}:{}", address, port);

            let mut to_visit = VecDeque::new();
            let mut visited = HashSet::new();

            let pages_dir = config::BASE_PATH.join("pages");
            let routes = routing::get_compiled_routes(&pages_dir);
            for route in routes {
                if route.regex.captures_len() <= 1 { // captures_len is number of groups + 1
                    let route_path = route.regex.to_string().trim_start_matches('^').trim_end_matches('$').to_string();
                    to_visit.push_back(route_path);
                }
            }

            while let Some(route_path) = to_visit.pop_front() {
                if visited.contains(&route_path) {
                    continue;
                }
                visited.insert(route_path.clone());

                let url = format!("{}{}", base_url, route_path);
                log::debug!("Rendering route: {}", url);

                let response = client.get(&url).send().await.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                let html_content = response.text().await.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                let html_content_relative = html_content.replace(&base_url, "");

                let document = scraper::Html::parse_document(&html_content);
                let selector = scraper::Selector::parse("a[href]").unwrap();

                for element in document.select(&selector) {
                    if let Some(href) = element.value().attr("href") {
                        if href.starts_with('/') {
                            log::info!("Found link: {}", href);
                            to_visit.push_back(href.to_string());
                        }
                    }
                }

                let mut file_path = msg.output_path.clone();
                let route_path_trimmed = route_path.trim_start_matches('/');
                let relative_path = PathBuf::from(route_path_trimmed);

                if route_path == "/" {
                    file_path.push("index.html");
                } else if relative_path.extension().is_some() {
                    file_path.push(relative_path);
                }
                else {
                    file_path.push(relative_path.join("index.html"));
                }

                let file_path_str = file_path.to_str().unwrap_or_default().replace('\\', "");
                let file_path = PathBuf::from(file_path_str);

                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&file_path, html_content_relative)?;
                log::info!("Saved page to: {:?}", file_path);
            }

            if let Some(static_path_str) = &crate::config::CONFIG.static_path {
                let static_path = if static_path_str.starts_with('/') {
                    Path::new(static_path_str).to_path_buf()
                } else {
                    crate::config::BASE_PATH.join(static_path_str)
                };
                if static_path.exists() {
                    log::info!("Copying static files from: {:?}", static_path);
                    let static_dir_name = Path::new(static_path_str).file_name().unwrap_or_else(|| std::ffi::OsStr::new("static"));
                    let output_static_path = msg.output_path.join(static_dir_name);
                    copy_dir_all(&static_path, output_static_path)?;
                    log::info!("Static files copied.");
                }
            }

            let static_dir_name = crate::config::CONFIG.static_path.as_deref()
                .map(|p| Path::new(p).file_name().unwrap_or_else(|| std::ffi::OsStr::new("static")))
                .unwrap_or_else(|| std::ffi::OsStr::new("static"));

            let noventa_static_path = msg.output_path.join(static_dir_name).join("noventa-static");
            fs::create_dir_all(&noventa_static_path)?;
            for (hash, file) in static_assets::EMBEDDED_FILES.iter() {
                let file_path = noventa_static_path.join(hash);
                fs::write(file_path, file.content)?;
            }

            log::info!("Static site generation finished successfully.");
            Ok(())
        })
    }
}