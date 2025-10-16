Any code that is not the webserver must be implemented using Actix actor model, structured in proper folders following best styles. Always add Unit Tests and End to End tests.

[dependencies]
actix = "0.13.5"
actix-web = "4.11.0"
pyo3 = "0.26.0"


# Features

## Toggleable Debug Information with `log` and `env_logger`

This project uses the `log` and `env_logger` crates to provide a flexible and configurable logging system. This allows for toggleable debug information that can be controlled at runtime without needing to recompile the application.

### How It Works

- **`log` crate**: Provides a standard logging API with macros such as `info!`, `debug!`, `warn!`, and `error!`.
- **`env_logger` crate**: An implementation of the `log` API that allows you to configure logging levels using the `RUST_LOG` environment variable.

### How to Use

You can control the verbosity of the log output by setting the `RUST_LOG` environment variable before running the application.

- **Show informational messages (and above):**
  ```bash
  RUST_LOG=info cargo run
  ```

- **Show debug messages for this specific crate:**
  ```bash
  RUST_LOG=noventa=debug cargo run
  ```

- **Show trace-level details for all crates:**
  ```bash
  RUST_LOG=trace cargo run
  ```

## Actor-Based Rendering and Request Orchestration

The framework utilizes an actor-based architecture to handle HTTP requests, process Python components, and render HTML templates. This design promotes separation of concerns, concurrency, and scalability.

### Core Actors

- **`HttpOrchestratorActor`**: Acts as the entry point for incoming web requests. It orchestrates the entire process by coordinating with the interpreter and renderer actors. For each request, it:
 1. Receives the request from the Actix Web router.
 2. Sends a message to the `InterpreterManager` to execute the relevant Python component.
 3. Once the data is received from the component, it sends a message to the `RendererActor` with the template name and the data context.
 4. Returns the final rendered HTML to the user.

- **`PythonInterpreterActor` / `InterpreterManager`**: A pool of actors responsible for executing Python code. Each actor runs in its own thread, allowing for concurrent processing of Python components. They receive a component path, execute the `render` function within it, and return the resulting data (as a JSON-serializable dictionary) to the orchestrator.

- **`RendererActor`**: This actor is responsible for rendering HTML templates using the `minijinja` templating engine. It receives a template name and a data context from the orchestrator, renders the template, and returns the resulting HTML string. It is configured to load templates from the `web/pages` directory.

### Request Lifecycle

1. An HTTP request hits a route defined in `routing.rs`.
2. The route handler forwards the request to the `HttpOrchestratorActor` by sending it a `HandleRequest` message.
3. The `HttpOrchestratorActor` sends a `RenderComponent` message to the `InterpreterManager`.
4. The `InterpreterManager` selects an available `PythonInterpreterActor` and forwards the message.
5. The `PythonInterpreterActor` executes the Python component's `render` function, which returns a dictionary of data.
6. The data is returned to the `HttpOrchestratorActor`.
7. The `HttpOrchestratorActor` sends a `RenderMessage` (containing the template name and the data) to the `RendererActor`.
8. The `RendererActor` uses `minijinja` to render the corresponding HTML template from `web/pages` with the provided data.
9. The rendered HTML is sent back to the `HttpOrchestratorActor`.
10. The `HttpOrchestratorActor` returns the HTML in an `HttpResponse` to the client.


## Component-Based Rendering

The framework now supports a component-based rendering system, allowing for the creation of reusable UI elements that can be embedded within pages and other components.

### Component Discovery

- **Automatic Scanning**: The framework automatically scans the `web/components` directory at startup to discover all available components.
- **Unique IDs**: Each component is assigned a unique ID based on its folder path. The path separator `/` is replaced with `.`. For example:
  - A component in `web/components/hello/` will have the ID `hello`.
  - A nested component in `web/components/hello/children/` will have the ID `hello.children`.
- **Template Naming**: The HTML template for a component must have the same name as its containing folder (e.g., `web/components/hello/hello.html`).

### How to Use Components

- **`component()` Function**: A `component()` function is available in the Jinja templates to render components.
- **Example**: To render the `hello` component, you would use the following syntax in your template:
  ```jinja
  {{ component("hello") }}
  ```

### Recursive and Context-Aware Rendering

- **Context Passing**: When a component is rendered, the current template's context is automatically passed down to the component's template. This allows components to access and use data from their parent.
- **Nested Components**: Because the context is passed down, components can be nested within each other, and the rendering will be handled recursively. For example, if `hello.html` contains `{{ component("hello.children") }}`, the `hello.children` component will be rendered with the same context available to `hello`.
