use actix::prelude::*;
use actix_web::{web, App, HttpServer};
use pyo3::types::{PyAnyMethods, PyListMethods};
use std::path::Path;
use crate::actors::page_renderer::RenderMessage;

mod actors;
mod components;
mod config;
mod dto;
mod fileupload;
mod routing;

use actors::health::HealthActor;
use actors::interpreter::PythonInterpreterActor;
use actors::load_shedding::LoadSheddingActor;
use actors::page_renderer::PageRendererActor;
use actors::template_renderer::TemplateRendererActor;
use actors::component_renderer::ComponentRendererActor;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    pyo3::Python::with_gil(|py| {
        let sys = py.import("sys").unwrap();
        let path = sys.getattr("path").unwrap();
        let path_list = path.downcast::<pyo3::types::PyList>().unwrap();
        if path_list.append("../database").is_err() {
            log::error!("Failed to add database to sys.path");
        }
        if path_list.append("../web").is_err() {
            log::error!("Failed to add web to sys.path");
        }
        if path_list.append("/Users/marcos/Documents/noventa/framework").is_err() {
            log::error!("Failed to add framework to sys.path");
        }
    });

    // Define the paths to the web directories
    let components_dir = Path::new("../web/components");
    let pages_dir = Path::new("../web/pages");

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
        PythonInterpreterActor::new(components_clone.clone())
    });

    // ComponentRendererActor is a lightweight coordinator, so it runs as a regular async actor.
    let component_renderer_addr = ComponentRendererActor::new(interpreters_addr.clone(), health_actor_addr.clone()).start();

    // TemplateRendererActor is also CPU-bound and runs in its own SyncArbiter.
    let value = health_actor_addr.clone();
    let template_renderer_addr = SyncArbiter::start(template_renderer_threads, move || {
        TemplateRendererActor::new(component_renderer_addr.clone(), value.clone())
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

    HttpServer::new(move || {
        let mut app = App::new()
            .app_data(renderer_data.clone())
            .app_data(web::Data::new(health_actor_addr.clone()))
            .route("/health", web::get().to(routing::health_check));

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
