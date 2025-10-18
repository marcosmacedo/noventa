use actix::prelude::*;
use actix_web::{web, App, HttpServer};
use std::path::Path;

mod actors;
mod components;
mod routing;

use actors::interpreter::PythonInterpreterActor;
use actors::page_renderer::PageRendererActor;
use actors::template_renderer::TemplateRendererActor;
use actors::component_renderer::ComponentRendererActor;

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

    // Build the routes from the pages directory
    let routes = routing::get_routes(pages_dir);

    // Create a pool of arbiters, each with its own Python interpreter actor
    let num_threads = 4; // Or use num_cpus::get()
    let components_clone = components.clone();
    let interpreters_addr = SyncArbiter::start(num_threads, move || PythonInterpreterActor::new(components_clone.clone()));

    // Start the component renderer actor
    let component_renderer_addr = ComponentRendererActor::new(interpreters_addr.clone()).start();

    // Start the template renderer actor in a SyncArbiter
    let template_renderer_addr = SyncArbiter::start(num_threads, move || TemplateRendererActor::new(component_renderer_addr.clone()));

    let renderer_addr = PageRendererActor::new(template_renderer_addr.clone()).start();


    let renderer_data = web::Data::new(renderer_addr);


    HttpServer::new(move || {
        let mut app = App::new()
            .app_data(renderer_data.clone());

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
                        routing::handle_page(req, payload, renderer_data_clone.clone(), template_path.clone())
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
