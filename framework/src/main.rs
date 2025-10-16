use actix::prelude::*;
use actix_web::{web, App, HttpServer};
use std::path::Path;

mod actors;
mod components;
mod routing;

use actors::interpreter::{LoadComponents, PythonInterpreterActor};
use actors::manager::InterpreterManager;
use actors::orchestrator::HttpOrchestratorActor;
use actors::renderer::RendererActor;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    pyo3::Python::initialize();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Define the paths to the web directories
    let components_dir = Path::new("../web/components");
    let pages_dir = Path::new("../web/pages");

    // Scan for components
    let components = components::scan_components(components_dir)?;
    log::info!("Found {} components.", components.len());

    // Create a pool of arbiters, each with its own Python interpreter actor
    let num_threads = 4; // Or dynamically get num_cpus
    let mut recipients = Vec::new();
    for _ in 0..num_threads {
        let arbiter = Arbiter::new();
        let addr = PythonInterpreterActor::start_in_arbiter(&arbiter.handle(), |_| {
            PythonInterpreterActor::new()
        });
        addr.do_send(LoadComponents {
            components: components.clone(),
        });
        recipients.push(addr.recipient());
    }

    // Start the interpreter manager actor
    let manager = InterpreterManager::new(recipients).start();

    // Start the renderer actor
    let renderer = RendererActor::new().start();

    // Start the orchestrator actor
    let orchestrator = HttpOrchestratorActor::new(manager.clone(), renderer).start();

    let components_data = web::Data::new(components);
    let orchestrator_data = web::Data::new(orchestrator);

    // Start the HTTP server
    let routes = routing::get_routes(pages_dir);
    HttpServer::new(move || {
        let mut app = App::new()
            .app_data(components_data.clone())
            .app_data(orchestrator_data.clone());

        for (route, template_path) in &routes {
            app = app.route(
                route,
                web::get().to({
                    let template_name = template_path
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string();
                    move |req, components, orchestrator| {
                        routing::handle_page(req, components, orchestrator, template_name.clone())
                    }
                }),
            );
        }
        app
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
