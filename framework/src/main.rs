pub mod scripts;
use actix::prelude::*;
use actix_session::Session;
use actix_web::{web, App, HttpRequest, HttpServer, Error, cookie::{Key, SameSite}, HttpResponse};
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
mod logger;
mod templates;
mod errors;
mod lsp;
mod static_assets;

use actors::health::HealthActor;
use actors::interpreter::PythonInterpreterActor;
use actors::load_shedding::LoadSheddingActor;
use actors::page_renderer::PageRendererActor;
use actors::template_renderer::TemplateRendererActor;
use actors::dev_websockets::DevWebSocket;
use actors::file_watcher::FileWatcherActor;
use actors::router::RouterActor;
use actors::ws_server::WsServer;
use actors::ssg::SSGActor;

use clap::Parser;

struct DevServerState {
    watcher: Addr<FileWatcherActor>,
    lsp: Addr<lsp::LspActor>,
}

#[derive(Parser)]
#[command(name = "noventa")]
#[command(about = "A framework for building web applications with Python and Rust.", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    #[clap(long, global = true, hide = true)]
    starter: Option<String>,
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
    New {
        #[clap(long, action)]
        no_input: bool,
    },
    Ssg {
        #[clap(long, action)]
        path: String,
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    let (dev_mode, command) = match &cli.command {
        Some(Commands::Dev) => (true, cli.command.as_ref()),
        Some(Commands::Serve) => (false, cli.command.as_ref()),
        Some(Commands::Disco) => (false, cli.command.as_ref()),
        Some(Commands::New { .. }) => (false, cli.command.as_ref()),
        Some(Commands::Ssg { .. }) => (true, cli.command.as_ref()),
        None => (false, None),
    };

    match command {
        Some(Commands::Dev) => {
            let server = run_dev_server().await?;
            server.await
        }
        Some(Commands::Serve) => {
            let server = run_prod_server().await?;
            server.await
        }
        Some(Commands::Disco) => disco::server::run_disco_server().await,
        Some(Commands::New { no_input }) => create_new_project(cli.starter.as_deref(), *no_input),
        Some(Commands::Ssg { path }) => {
            let srv = run_dev_server().await?;
            let srv_handle = srv.handle();
            let ssg_actor = SSGActor::new().start();

            tokio::spawn(srv);

            let res = ssg_actor.send(actors::ssg::SsgMessage { output_path: path.into() }).await;

            if let Err(e) = res {
                log::error!("SSG actor mailbox error: {}", e);
            }
            
            srv_handle.stop(true).await;
            log::info!("Server stopped. Exiting.");
            Ok(())
        }
        None => {
            use clap::CommandFactory;
            Cli::command().print_help()?;
            Ok(())
        }
    }
}

fn create_new_project(starter_path: Option<&str>, no_input: bool) -> std::io::Result<()> {
    let template_path = if let Some(path) = starter_path {
        Path::new(path).to_path_buf()
    } else {
        // Fallback for local development: find the `starter` directory relative to the executable.
        let mut exe_path = env::current_exe()?;
        exe_path.pop();
        exe_path.join("starter")
    };

    if !template_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Template directory not found at: {:?}", template_path),
        ));
    }

    let output_dir = ".";

    let mut command = Command::new("python");
    command.arg("-m").arg("cookiecutter").arg(template_path).arg("--output-dir").arg(output_dir);

    if no_input {
        command.arg("--no-input");
    }

    let status = command.status()?;

    if !status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to execute cookiecutter command",
        ));
    }

    println!("✨ Your new project has been created successfully! Happy coding!");
    Ok(())
}

async fn run_dev_server() -> std::io::Result<actix_web::dev::Server> {
    let (
        health_actor_addr,
        renderer_data,
        interpreters_addr,
        template_renderer_addr,
        actix_web_threads,
        runtime_store,
        runtime_secret,
    ) = configure_server(true).await?;

    let router_addr = RouterActor::new().start();
    let ws_server = WsServer::new().start();
    let watcher = FileWatcherActor::new(
        ws_server.clone(),
        router_addr.clone(),
        template_renderer_addr.clone(),
        interpreters_addr.clone(),
    )
    .start();
    let lsp_actor = lsp::LspActor.start();

    let server_state = web::Data::new(DevServerState {
        watcher,
        lsp: lsp_actor,
    });

    let server = HttpServer::new(move || {
        let mut app = App::new()
            .wrap(actix_web::middleware::Compress::default())
            .app_data(server_state.clone())
            .app_data(renderer_data.clone())
            .app_data(web::Data::new(health_actor_addr.clone()))
            .app_data(web::Data::new(true))
            .route("/health", web::get().to(routing::health_check))
            .app_data(web::Data::new(router_addr.clone()))
            .app_data(web::Data::new(ws_server.clone()))
            .route("/devws", web::get().to(dev_ws))
            .route("/noventa-static/{filename:.*}", web::get().to(serve_embedded_file))
            .default_service(web::route().to(routing::dynamic_route_handler));

        if let Some(static_path_str) = &config::CONFIG.static_path {
            let static_path = if static_path_str.starts_with('/') {
                std::path::PathBuf::from(static_path_str).clean()
            } else {
                config::BASE_PATH.join(static_path_str).clean()
            };
            let url_prefix = config::CONFIG
                .static_url_prefix
                .as_deref()
                .unwrap_or("/static");
            app = app.service(Files::new(url_prefix, static_path));
        }

        app.wrap(
            SessionMiddleware::builder(runtime_store.clone(), runtime_secret.clone())
                .cookie_name(
                    config::CONFIG
                        .session
                        .as_ref()
                        .map(|s| s.cookie_name.clone())
                        .unwrap_or_else(|| "noventa_session".to_string()),
                )
                .cookie_secure(
                    config::CONFIG
                        .session
                        .as_ref()
                        .map(|s| s.cookie_secure)
                        .unwrap_or(false),
                )
                .cookie_http_only(
                    config::CONFIG
                        .session
                        .as_ref()
                        .map(|s| s.cookie_http_only)
                        .unwrap_or(true),
                )
                .cookie_path(
                    config::CONFIG
                        .session
                        .as_ref()
                        .map(|s| s.cookie_path.clone())
                        .unwrap_or_else(|| "/".to_string()),
                )
                .cookie_same_site(SameSite::Lax)
                .cookie_domain(
                    config::CONFIG
                        .session
                        .as_ref()
                        .and_then(|s| s.cookie_domain.clone()),
                )
                .session_lifecycle(
                    PersistentSession::default().session_ttl(
                        config::CONFIG
                            .session
                            .as_ref()
                            .and_then(|s| {
                                s.cookie_max_age
                                    .map(actix_web::cookie::time::Duration::seconds)
                            })
                            .unwrap_or(actix_web::cookie::time::Duration::days(7)),
                    ),
                )
                .build(),
        )
    })
    .workers(actix_web_threads)
    .keep_alive(std::time::Duration::from_secs(30))
    .bind({
        let port = config::CONFIG.port.unwrap_or(8080);
        if port > 65535 {
            println!(
                "Error: Port number {} is too high. It must be between 0 and 65535.",
                port
            );
            std::process::exit(1);
        }
        (
            config::CONFIG.server_address.as_deref().unwrap_or("127.0.0.1"),
            port as u16,
        )
    })
    .map_err(|e| {
        if e.kind() == std::io::ErrorKind::AddrInUse {
            let port = config::CONFIG.port.unwrap_or(8080) as u16;
            println!("Error: The port {} is already in use.", port);
            println!("Another application is likely running on this port.");
            println!("Please stop the other application or choose a different port.");
            std::process::exit(1);
        }
        e
    })?;

    logger::print_banner(
        config::CONFIG.server_address.as_deref().unwrap_or("127.0.0.1"),
        config::CONFIG.port.unwrap_or(8080) as u16,
        true,
    );

    Ok(server.run())
}

async fn configure_server(
    dev_mode: bool,
) -> std::io::Result<(
    Addr<HealthActor>,
    web::Data<Recipient<RenderMessage>>,
    Addr<PythonInterpreterActor>,
    Addr<TemplateRendererActor>,
    usize,
    session::RuntimeSessionStore,
    Key,
)> {
    let log_level = config::CONFIG
        .log_level
        .as_deref()
        .unwrap_or(if dev_mode { "info" } else { "warn" });
    logger::init_logger(log_level);

    let components_dir = Path::new("./components");
    let components = components::scan_components(components_dir)?;
    log::debug!("Found {} components. Ready to roll!", components.len());

    let total_cores = num_cpus::get();
    let (python_threads, template_renderer_threads, actix_web_threads) =
        if let Some(core_config) = &config::CONFIG.core_allocation {
            let python_threads = core_config.python_threads.unwrap_or((total_cores / 2).max(1));
            let template_renderer_threads = core_config
                .template_renderer_threads
                .unwrap_or((total_cores / 2).max(1));
            let actix_web_threads = core_config.actix_web_threads.unwrap_or(
                (total_cores - python_threads - template_renderer_threads).max(1),
            );
            (
                python_threads,
                template_renderer_threads,
                actix_web_threads,
            )
        } else {
            let python_threads = (total_cores / 2).max(1);
            let template_renderer_threads = (total_cores / 2).max(1);
            let actix_web_threads =
                (total_cores - python_threads - template_renderer_threads).max(1);
            (
                python_threads,
                template_renderer_threads,
                actix_web_threads,
            )
        };

    log::debug!(
        "Core allocation: Total={}, Actix Web={}, Python={}, Template Renderer={}. Starting up the engines!",
        total_cores,
        actix_web_threads,
        python_threads,
        template_renderer_threads
    );

    let health_actor_addr = HealthActor::new().start();
    let interpreters_addr =
        SyncArbiter::start(python_threads, move || PythonInterpreterActor::new(dev_mode));
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

    let page_renderer_addr =
        PageRendererActor::new(template_renderer_addr.clone(), health_actor_addr.clone()).start();
    let load_shedding_actor =
        LoadSheddingActor::new(page_renderer_addr.clone(), health_actor_addr.clone()).start();

    let renderer_data: web::Data<Recipient<RenderMessage>> =
        if config::CONFIG.adaptive_shedding.unwrap_or(true) {
            log::debug!("Adaptive load shedding is enabled. The server will automatically adjust to traffic spikes.");
            web::Data::new(load_shedding_actor.recipient())
        } else {
            log::debug!("Adaptive load shedding is disabled. The server will handle all requests without throttling.");
            web::Data::new(page_renderer_addr.recipient())
        };

    use std::sync::Arc as StdArc;
    let (runtime_store, runtime_secret): (session::RuntimeSessionStore, Key) =
        if let Some(session_config) = &config::CONFIG.session {
            let secret_key_bytes = session_config.secret_key.as_bytes();
            let secret_key = match Key::try_from(secret_key_bytes) {
                Ok(key) => key,
                Err(e) => {
                    println!("Your `secret_key` in `config.yaml` is not long enough. It needs to be at least 64 characters long for security. Please generate a new, longer key.");
                    println!("Details: {}", e);
                    std::process::exit(1);
                }
            };
            let store = match session_config.backend {
                config::SessionBackend::Cookie => session::RuntimeSessionStore::Cookie(
                    StdArc::new(CookieSessionStore::default()),
                ),
                config::SessionBackend::Memory => {
                    session::RuntimeSessionStore::InMemory(session::InMemoryBackend::new())
                }
                config::SessionBackend::Redis => {
                    let redis_url = session_config
                        .redis_url
                        .as_ref()
                        .expect("redis_url is required for redis session backend");
                    let redis_pool_size = session_config.redis_pool_size.unwrap_or(10) as usize;
                    let mut redis_cfg = Config::from_url(redis_url);
                    redis_cfg.pool = Some(deadpool_redis::PoolConfig {
                        max_size: redis_pool_size,
                        ..Default::default()
                    });
                    let redis_pool = redis_cfg
                        .create_pool(Some(Runtime::Tokio1))
                        .expect("Failed to create redis pool");
                    let store = RedisSessionStore::new_pooled(redis_pool)
                        .await
                        .expect("Failed to create Redis session store");
                    session::RuntimeSessionStore::Redis(store)
                }
            };
            (store, secret_key)
        } else {
            let secret_key = Key::from(&[0u8; 64]);
            log::warn!("Heads up! No session key was found in your `config.yaml`. We're using a temporary key for now, but for production, you'll want to set a secure `secret_key`.");
            let store = session::RuntimeSessionStore::Cookie(StdArc::new(
                CookieSessionStore::default(),
            ));
            (store, secret_key)
        };

    Ok((
        health_actor_addr,
        renderer_data,
        interpreters_addr,
        template_renderer_addr,
        actix_web_threads,
        runtime_store,
        runtime_secret,
    ))
}

async fn dev_ws(req: HttpRequest, stream: web::Payload, srv: web::Data<Addr<WsServer>>) -> Result<actix_web::HttpResponse, Error> {
    ws::start(DevWebSocket::new(srv.get_ref().clone()), &req, stream)
}

async fn run_prod_server() -> std::io::Result<actix_web::dev::Server> {
    let (
        health_actor_addr,
        renderer_data,
        _,
        _,
        actix_web_threads,
        runtime_store,
        runtime_secret,
    ) = configure_server(false).await?;

    let server = HttpServer::new(move || {
        let mut app = App::new()
            .wrap(actix_web::middleware::Compress::default())
            .app_data(renderer_data.clone())
            .app_data(web::Data::new(health_actor_addr.clone()))
            .app_data(web::Data::new(false))
            .route("/health", web::get().to(routing::health_check))
            .route("/noventa-static/{filename:.*}", web::get().to(serve_embedded_file));

        let pages_dir = config::BASE_PATH.join("pages");
        let routes = routing::get_compiled_routes(&pages_dir);
        for route in routes {
            let template_path = route.template_path.to_str().unwrap().to_string();
            let route_pattern = route
                .regex
                .to_string()
                .trim_start_matches('^')
                .trim_end_matches('$')
                .to_string();
            app = app.route(
                &route_pattern,
                web::route().to(
                    move |req: HttpRequest,
                          payload: web::Payload,
                          renderer: web::Data<Recipient<RenderMessage>>,
                          session: Session,
                          path_params: web::Path<std::collections::HashMap<String, String>>| {
                        let template_path_clone = template_path.clone();
                        async move {
                            routing::handle_page_native(
                                req,
                                payload,
                                renderer,
                                session,
                                path_params,
                                web::Data::new(template_path_clone),
                            )
                            .await
                        }
                    },
                ),
            );
        }

        if let Some(static_path_str) = &config::CONFIG.static_path {
            let static_path = if static_path_str.starts_with('/') {
                std::path::PathBuf::from(static_path_str).clean()
            } else {
                config::BASE_PATH.join(static_path_str).clean()
            };
            let url_prefix = config::CONFIG
                .static_url_prefix
                .as_deref()
                .unwrap_or("/static");
            app = app.service(Files::new(url_prefix, static_path));
        }

        app.wrap(
            SessionMiddleware::builder(runtime_store.clone(), runtime_secret.clone())
                .cookie_name(
                    config::CONFIG
                        .session
                        .as_ref()
                        .map(|s| s.cookie_name.clone())
                        .unwrap_or_else(|| "noventa_session".to_string()),
                )
                .cookie_secure(
                    config::CONFIG
                        .session
                        .as_ref()
                        .map(|s| s.cookie_secure)
                        .unwrap_or(false),
                )
                .cookie_http_only(
                    config::CONFIG
                        .session
                        .as_ref()
                        .map(|s| s.cookie_http_only)
                        .unwrap_or(true),
                )
                .cookie_path(
                    config::CONFIG
                        .session
                        .as_ref()
                        .map(|s| s.cookie_path.clone())
                        .unwrap_or_else(|| "/".to_string()),
                )
                .cookie_same_site(SameSite::Lax)
                .cookie_domain(
                    config::CONFIG
                        .session
                        .as_ref()
                        .and_then(|s| s.cookie_domain.clone()),
                )
                .session_lifecycle(
                    PersistentSession::default().session_ttl(
                        config::CONFIG
                            .session
                            .as_ref()
                            .and_then(|s| {
                                s.cookie_max_age
                                    .map(actix_web::cookie::time::Duration::seconds)
                            })
                            .unwrap_or(actix_web::cookie::time::Duration::days(7)),
                    ),
                )
                .build(),
        )
    })
    .workers(actix_web_threads)
    .keep_alive(std::time::Duration::from_secs(30))
    .bind({
        let port = config::CONFIG.port.unwrap_or(8080);
        if port > 65535 {
            println!(
                "Error: Port number {} is too high. It must be between 0 and 65535.",
                port
            );
            std::process::exit(1);
        }
        (
            config::CONFIG.server_address.as_deref().unwrap_or("127.0.0.1"),
            port as u16,
        )
    })
    .map_err(|e| {
        if e.kind() == std::io::ErrorKind::AddrInUse {
            let port = config::CONFIG.port.unwrap_or(8080) as u16;
            println!("Error: The port {} is already in use.", port);
            println!("Another application is likely running on this port.");
            println!("Please stop the other application or choose a different port.");
            std::process::exit(1);
        }
        e
    })?;

    logger::print_banner(
        config::CONFIG.server_address.as_deref().unwrap_or("127.0.0.1"),
        config::CONFIG.port.unwrap_or(8080) as u16,
        false,
    );

    Ok(server.run())
}

async fn serve_embedded_file(path: web::Path<String>) -> HttpResponse {
    let filename = path.into_inner();
    match static_assets::EMBEDDED_FILES.get(&filename) {
        Some(file) => HttpResponse::Ok()
            .content_type(file.content_type)
            .body(file.content),
        None => HttpResponse::NotFound().finish(),
    }
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
        
        let result = create_new_project(None, true);
        assert!(result.is_ok());
        
        std::env::set_current_dir(current_exe.parent().unwrap().parent().unwrap().parent().unwrap().parent().unwrap()).unwrap();
    }
}

