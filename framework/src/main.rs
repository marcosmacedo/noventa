use actix::prelude::*;
use actix_web::{web, App, HttpRequest, HttpServer, Error};
use actix_web_actors::ws;
use actix_files::Files;
use pyo3::types::{PyAnyMethods, PyListMethods};
use std::path::Path;
use std::process::Command;
use std::env;
use crate::actors::page_renderer::RenderMessage;

mod actors;
mod components;
mod config;
mod dto;
mod fileupload;
mod routing;
mod disco;

use actors::health::HealthActor;
use actors::interpreter::PythonInterpreterActor;
use actors::load_shedding::LoadSheddingActor;
use actors::page_renderer::PageRendererActor;
use actors::template_renderer::TemplateRendererActor;
use actors::component_renderer::ComponentRendererActor;
use actors::dev_websockets::DevWebSocket;
use actors::file_watcher::FileWatcherActor;
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
    let pages_dir = Path::new("./pages");

    // Scan for components
    let components = components::scan_components(components_dir)?;
    log::info!("Found {} components.", components.len());

    // Build the routes from the pages directory
    let routes = routing::get_routes(pages_dir);

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

    // ComponentRendererActor is a lightweight coordinator, so it runs as a regular async actor.
    let component_renderer_addr = ComponentRendererActor::new(interpreters_addr.clone(), health_actor_addr.clone()).start();

    // TemplateRendererActor is also CPU-bound and runs in its own SyncArbiter.
    let value = health_actor_addr.clone();
    let template_renderer_addr = SyncArbiter::start(template_renderer_threads, move || {
        TemplateRendererActor::new(component_renderer_addr.clone(), value.clone(), dev_mode)
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

    let mut ws_server: Option<Addr<WsServer>> = None;
    let mut _watcher: Option<Addr<FileWatcherActor>> = None;

    if dev_mode {
        let server = WsServer::new().start();
        _watcher = Some(FileWatcherActor::new(server.clone()).start());
        ws_server = Some(server);
    }

    HttpServer::new(move || {
        let mut app = App::new()
            .app_data(renderer_data.clone())
            .app_data(web::Data::new(health_actor_addr.clone()))
            .route("/health", web::get().to(routing::health_check));

        if dev_mode {
            app = app.app_data(web::Data::new(ws_server.as_ref().unwrap().clone()))
                     .route("/devws", web::get().to(dev_ws));
        }

        if let Some(static_path) = &config::CONFIG.static_path {
            let url_prefix = config::CONFIG
                .static_url_prefix
                .as_deref()
                .unwrap_or("/static");
            app = app.service(Files::new(url_prefix, static_path).show_files_listing());
        }

        for (route, template_path) in &routes {
            let renderer_data_clone = renderer_data.clone();
            app = app.route(
                route,
                web::route().to({
                    let template_path = template_path
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string();
                    move |req, payload| {
                        routing::handle_page(
                            req,
                            payload,
                            renderer_data_clone.clone(),
                            template_path.clone(),
                        )
                    }
                }),
            );
        }
        app
    })
    .workers(actix_web_threads) // Set the number of web server worker threads.
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}

async fn dev_ws(req: HttpRequest, stream: web::Payload, srv: web::Data<Addr<WsServer>>) -> Result<actix_web::HttpResponse, Error> {
    ws::start(DevWebSocket::new(srv.get_ref().clone()), &req, stream)
}

