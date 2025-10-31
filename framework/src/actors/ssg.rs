use actix::prelude::*;
use std::path::{Path, PathBuf};
use crate::routing;
use std::fs;
use std::io;

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

            let pages_dir = Path::new("./pages");
            let routes = routing::get_compiled_routes(pages_dir);
            let client = reqwest::Client::builder()
                .danger_accept_invalid_certs(true)
                .build()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            for route in routes {
                if route.regex.captures_len() > 1 { // captures_len is number of groups + 1
                    log::warn!("Skipping route with parameters: {}", route.regex.as_str());
                    continue;
                }

                let route_path = route.regex.to_string().trim_start_matches('^').trim_end_matches('$').to_string();
                let address = crate::config::CONFIG.server_address.as_deref().unwrap_or("127.0.0.1");
                let port = crate::config::CONFIG.port.unwrap_or(8080);
                let url = format!("http://{}:{}{}", address, port, route_path);
                log::info!("Rendering route: {}", url);

                let response = client.get(&url).send().await.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                let body = response.text().await.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

                let mut file_path = msg.output_path.clone();
                if route_path == "/" {
                    file_path.push("index.html");
                } else {
                    let mut relative_path = PathBuf::from(route_path.trim_start_matches('/'));
                    if relative_path.extension().is_none() {
                        relative_path.set_extension("html");
                    }
                    file_path.push(relative_path);
                }
    
                let file_path_str = file_path.to_str().unwrap_or_default().replace('\\', "");
                let file_path = PathBuf::from(file_path_str);
    
                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&file_path, body)?;
                log::info!("Saved page to: {:?}", file_path);
            }

            if let Some(static_path_str) = &crate::config::CONFIG.static_path {
                let static_path = Path::new(static_path_str);
                if static_path.exists() {
                    log::info!("Copying static files from: {:?}", static_path);
                    let static_dir_name = static_path.file_name().unwrap_or_else(|| std::ffi::OsStr::new("static"));
                    let output_static_path = msg.output_path.join(static_dir_name);
                    copy_dir_all(static_path, output_static_path)?;
                    log::info!("Static files copied.");
                }
            }

            log::info!("Static site generation finished successfully.");
            Ok(())
        })
    }
}