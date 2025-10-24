use actix::prelude::*;
use actix_web::{web, App, HttpRequest, HttpServer, Error, cookie::{Key, SameSite}};
use actix_session::config::PersistentSession;
use actix_session::{
    storage::{CookieSessionStore, RedisSessionStore},
    SessionMiddleware,
};
use actix_web_actors::ws;
use deadpool_redis::{Config, Runtime};
use actix_files::Files;
use pyo3::types::{PyAnyMethods, PyListMethods};
use std::path::Path;
use std::process::Command;
use path_clean::PathClean;
use std::env;
use crate::actors::page_renderer::RenderMessage;

mod actors;
pub mod components;
mod config;
mod dto;
mod fileupload;
mod routing;
mod disco;
mod session;

use actors::health::HealthActor;
use actors::interpreter::PythonInterpreterActor;
use actors::load_shedding::LoadSheddingActor;
use actors::page_renderer::PageRendererActor;
use actors::template_renderer::TemplateRendererActor;
use actors::dev_websockets::DevWebSocket;
use actors::file_watcher::FileWatcherActor;
use actors::router::RouterActor;
use actors::ws_server::WsServer;

use clap::Parser;

#[derive(Parser)]
#[command(name = "noventa")]
#[command(about = "A framework for building web applications with Python and Rust.", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Runs the development web server
    Dev,
    /// Runs the production web server
    Serve,
    /// Runs the MCP server
    Disco,
    /// Create a new project
    New { name: String },
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    let (dev_mode, command) = match cli.command {
        Some(Commands::Dev) => (true, Some(Commands::Dev)),
        Some(Commands::Serve) => (false, Some(Commands::Serve)),
        Some(Commands::Disco) => (false, Some(Commands::Disco)),
        Some(Commands::New { ref name }) => (false, Some(Commands::New { name: name.clone() })),
        None => (true, None), // Default to dev mode
    };

    let command_to_run = command.as_ref().or(cli.command.as_ref());

    match command_to_run {
        Some(Commands::Dev) | None => run_dev_server(dev_mode).await,
        Some(Commands::Serve) => run_dev_server(dev_mode).await,
        Some(Commands::Disco) => disco::server::run_disco_server().await,
        Some(Commands::New { name }) => create_new_project(name),
    }
}

fn create_new_project(name: &str) -> std::io::Result<()> {
    // Get the path to the currently running executable
    let mut exe_path = env::current_exe()?;
    // Navigate up to the project root (assuming the executable is in `noventa/framework/target/debug/noventa`)
    exe_path.pop(); // -> noventa/framework/target/debug
    exe_path.pop(); // -> noventa/framework/target
    exe_path.pop(); // -> noventa/framework
    exe_path.pop(); // -> noventa/

    // Now construct the path to the template
    let template_path = exe_path.join("framework/starter");

    if !template_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Template directory not found at: {:?}", template_path),
        ));
    }

    let output_dir = ".";

    let status = Command::new("cookiecutter")
        .arg(template_path)
        .arg("--output-dir")
        .arg(output_dir)
        .arg("--no-input")
        .arg(format!("project_name={}", name))
        .status()?;

    if !status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to execute cookiecutter command",
        ));
    }

    println!("Successfully created project: {}", name);
    Ok(())
}

async fn run_dev_server(dev_mode: bool) -> std::io::Result<()> {
    if dev_mode {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    } else {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    }

    pyo3::Python::attach(|py| {
        let sys = py.import("sys").unwrap();
        let path = sys.getattr("path").unwrap();
        let path_list = path.downcast::<pyo3::types::PyList>().unwrap();

        // FIXME: Needs to point to the installation path
        if path_list.append("/Users/marcos/Documents/noventa/framework").is_err() {
            log::error!("Failed to add framework to sys.path");
        }
        Ok::<(), pyo3::PyErr>(())
    }).unwrap();

    // Define the paths to the web directories
    let components_dir = Path::new("./components");

    // Scan for components
    let components = components::scan_components(components_dir)?;
    log::info!("Found {} components.", components.len());

    // --- Core Allocation ---
    // We are manually partitioning the CPU cores to ensure predictable performance.
    let total_cores = num_cpus::get();

    // Allocate cores for CPU-bound sync actors.
    // These run in dedicated thread pools (SyncArbiters).
    let python_threads = (total_cores / 2).max(1); // Example: 50% of cores for Python, at least 1.
    let template_renderer_threads = (total_cores / 2).max(1); // Example: 50% of cores for templates, at least 1.

    // Allocate the remaining cores to the Actix web server for handling I/O.
    let actix_web_threads = (total_cores - python_threads - template_renderer_threads).max(1);

    log::info!(
        "Core allocation: Total={}, Actix Web={}, Python={}, Template Renderer={}",
        total_cores,
        actix_web_threads,
        python_threads,
        template_renderer_threads
    );

    // --- Actor Initialization ---

    let health_actor_addr = HealthActor::new().start();

    // PythonInterpreterActor runs in a SyncArbiter with a dedicated thread pool.
    let components_clone = components.clone();
    let interpreters_addr = SyncArbiter::start(python_threads, move || {
        PythonInterpreterActor::new(components_clone.clone(), dev_mode)
    });

    // TemplateRendererActor is also CPU-bound and runs in its own SyncArbiter.
    let value = health_actor_addr.clone();
    let components_clone_for_template_renderer = components.clone();
    let interpreters_addr_clone = interpreters_addr.clone();
    let template_renderer_addr = SyncArbiter::start(template_renderer_threads, move || {
        TemplateRendererActor::new(
            interpreters_addr_clone.clone(),
            value.clone(),
            dev_mode,
            components_clone_for_template_renderer.clone(),
        )
    });

    // PageRendererActor is a lightweight coordinator, running as a regular async actor.
    let page_renderer_addr = PageRendererActor::new(template_renderer_addr.clone(), health_actor_addr.clone()).start();

    // Wrap the PageRendererActor with the LoadSheddingActor.
    let load_shedding_actor =
        LoadSheddingActor::new(page_renderer_addr.clone(), health_actor_addr.clone()).start();

    let renderer_data: web::Data<Recipient<RenderMessage>>;
    if config::CONFIG.adaptive_shedding.unwrap_or(true) {
        log::info!("Adaptive load shedding is ENABLED.");
        renderer_data = web::Data::new(load_shedding_actor.recipient());
    } else {
        log::info!("Adaptive load shedding is DISABLED.");
        renderer_data = web::Data::new(page_renderer_addr.recipient());
    };

    let router_addr = RouterActor::new().start();

    let mut ws_server: Option<Addr<WsServer>> = None;
    let mut _watcher: Option<Addr<FileWatcherActor>> = None;

    if dev_mode {
        let server = WsServer::new().start();
        _watcher = Some(FileWatcherActor::new(server.clone(), router_addr.clone(), template_renderer_addr.clone(), interpreters_addr.clone()).start());
        ws_server = Some(server);
    }

    // Prepare a runtime session store and secret key. If session config is missing,
    // we fall back to a default cookie store and a fixed key. This keeps the
    // middleware type consistent across configurations.
    use std::sync::Arc as StdArc;
    let (runtime_store, runtime_secret): (session::RuntimeSessionStore, Key) = if let Some(session_config) = &config::CONFIG.session {
        let secret_key = Key::from(session_config.secret_key.as_bytes());
        let store = match session_config.backend {
            config::SessionBackend::Cookie => {
                session::RuntimeSessionStore::Cookie(StdArc::new(CookieSessionStore::default()))
            }
            config::SessionBackend::InMemory => {
                session::RuntimeSessionStore::InMemory(session::InMemoryBackend::new())
            }
            config::SessionBackend::Redis => {
                let redis_url = session_config.redis_url.as_ref().expect("redis_url is required for redis session backend");
                let redis_pool_size = session_config.redis_pool_size.unwrap_or(10) as usize;
                // Create config from URL and set pool max size
                let mut redis_cfg = Config::from_url(redis_url);
                redis_cfg.pool = Some(deadpool_redis::PoolConfig {
                    max_size: redis_pool_size,
                    ..Default::default()
                });
                let redis_pool = redis_cfg.create_pool(Some(Runtime::Tokio1)).expect("Failed to create redis pool");
                let store = RedisSessionStore::new_pooled(redis_pool).await.expect("Failed to create Redis session store");
                session::RuntimeSessionStore::Redis(store)
            }
        };
        (store, secret_key)
    } else {
        // Default fallback if no session config is provided
        let secret_key = Key::from(&[0u8; 64]); // Use a secure, random key in production
        log::warn!("No session configuration found. Using default cookie session store with a fixed secret key. This is NOT recommended for production.");
        let store = session::RuntimeSessionStore::Cookie(StdArc::new(CookieSessionStore::default()));
        (store, secret_key)
    };

    HttpServer::new(move || {
        let mut app = App::new()
            .wrap(actix_web::middleware::Compress::default())
            .app_data(renderer_data.clone())
            .app_data(web::Data::new(health_actor_addr.clone()))
            .app_data(web::Data::new(router_addr.clone()))
            .route("/health", web::get().to(routing::health_check));

        if dev_mode {
            app = app.app_data(web::Data::new(ws_server.as_ref().unwrap().clone()))
                     .route("/devws", web::get().to(dev_ws));
        }

        if let Some(static_path_str) = &config::CONFIG.static_path {
            let static_path = std::path::PathBuf::from(static_path_str).clean();
            let url_prefix = config::CONFIG
                .static_url_prefix
                .as_deref()
                .unwrap_or("/static");
            app = app.service(Files::new(url_prefix, static_path));
        }

        app = app.default_service(web::route().to(routing::dynamic_route_handler));

        // Always wrap with the session middleware using the runtime-configured store.
        app.wrap(
            SessionMiddleware::builder(runtime_store.clone(), runtime_secret.clone())
                .cookie_name(
                    config::CONFIG.session.as_ref()
                        .map(|s| s.cookie_name.clone())
                        .unwrap_or_else(|| "noventa_session".to_string()),
                )
                .cookie_secure(config::CONFIG.session.as_ref().map(|s| s.cookie_secure).unwrap_or(false))
                .cookie_http_only(config::CONFIG.session.as_ref().map(|s| s.cookie_http_only).unwrap_or(true))
                .cookie_path(
                    config::CONFIG.session.as_ref()
                        .map(|s| s.cookie_path.clone())
                        .unwrap_or_else(|| "/".to_string()),
                )
                .cookie_same_site(SameSite::Lax)
                .cookie_domain(
                    config::CONFIG.session.as_ref()
                        .and_then(|s| s.cookie_domain.clone())
                )
                .session_lifecycle(
                    PersistentSession::default().session_ttl(
                        config::CONFIG.session.as_ref()
                            .and_then(|s| s.cookie_max_age.map(actix_web::cookie::time::Duration::seconds))
                            .unwrap_or(actix_web::cookie::time::Duration::days(7))
                    )
                )
                .build(),
        )
    })
    .workers(actix_web_threads) // Set the number of web server worker threads.
    .keep_alive(std::time::Duration::from_secs(30))
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}

async fn dev_ws(req: HttpRequest, stream: web::Payload, srv: web::Data<Addr<WsServer>>) -> Result<actix_web::HttpResponse, Error> {
    ws::start(DevWebSocket::new(srv.get_ref().clone()), &req, stream)
}



#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::tempdir;

    #[test]
    #[ignore]
    fn test_create_new_project() {
        let dir = tempdir().unwrap();
        let project_name = "test_project";
        let project_path = dir.path().join(project_name);

        // Create a dummy template directory
        let template_dir = dir.path().join("framework/starter");
        fs::create_dir_all(&template_dir).unwrap();
        File::create(template_dir.join("cookiecutter.json")).unwrap();

        // Mock the executable path
        let mut exe_path = dir.path().to_path_buf();
        exe_path.push("framework");
        exe_path.push("target");
        exe_path.push("debug");
        exe_path.push("noventa");
        fs::create_dir_all(exe_path.parent().unwrap()).unwrap();
        File::create(&exe_path).unwrap();
        let current_exe = std::env::current_exe().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        
        let result = create_new_project(project_name);
        assert!(result.is_ok());
        
        std::env::set_current_dir(current_exe.parent().unwrap().parent().unwrap().parent().unwrap().parent().unwrap()).unwrap();
    }
}
