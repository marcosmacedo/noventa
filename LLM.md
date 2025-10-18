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
 3. Once the data is received from the component, it sends a message to the `PageRendererActor` with the template name and the data context.
 4. Returns the final rendered HTML to the user.

- **`PythonInterpreterActor` / `InterpreterManager`**: A pool of actors responsible for executing Python code. Each actor runs in its own thread, allowing for concurrent processing of Python components. They receive a component path, execute the `render` function within it, and return the resulting data (as a JSON-serializable dictionary) to the orchestrator.

- **`PageRendererActor`**: This actor is responsible for rendering HTML templates using the `minijinja` templating engine. It receives a template name and a data context from the orchestrator, renders the template, and returns the resulting HTML string. It is configured to load templates from the `web/pages` directory.

### Request Lifecycle

1. An HTTP request hits a route defined in `routing.rs`.
2. The route handler forwards the request to the `HttpOrchestratorActor` by sending it a `HandleRequest` message.
3. The `HttpOrchestratorActor` sends a `RenderComponent` message to the `InterpreterManager`.
4. The `InterpreterManager` selects an available `PythonInterpreterActor` and forwards the message.
5. The `PythonInterpreterActor` executes the Python component's `render` function, which returns a dictionary of data.
6. The data is returned to the `HttpOrchestratorActor`.
7. The `HttpOrchestratorActor` sends a `RenderMessage` (containing the template name and the data) to the `PageRendererActor`.
8. The `PageRendererActor` uses `minijinja` to render the corresponding HTML template from `web/pages` with the provided data.
9. The rendered HTML is sent back to the `HttpOrchestratorActor`.
10. The `HttpOrchestratorActor` returns the HTML in an `HttpResponse` to the client.


## Component-Based Rendering

The framework now supports a component-based rendering system, allowing for the creation of reusable UI elements that can be embedded within pages and other components.

### Component Discovery

- **Automatic Scanning**: The framework automatically scans the `web/components` directory at startup to discover all available components. Each direct subdirectory within `web/components` is considered a component.
- **Unique IDs**: Each component is assigned a unique ID based on its folder name. For example, a component in `web/components/counter/` will have the ID `counter`.
- **File Structure**: Each component directory must adhere to the following rules:
  - It can contain **at most one** `.html` file, which serves as its template.
  - It can contain **at most one** `.py` file, which provides its server-side logic.
  - The names of the `.html` and `.py` files do not need to match the component's directory name.
- **Error Handling**: If a component directory contains more than one `.html` or `.py` file, an error will be logged, and the component will be skipped.

### How to Use Components

- **`component()` Function**: A `component()` function is available in the Jinja templates to render components.
- **Example**: To render the `hello` component, you would use the following syntax in your template:
  ```jinja
  {{ component("hello") }}
  ```

### Recursive and Context-Aware Rendering

- **Context Passing**: When a component is rendered, the current template's context is automatically passed down to the component's template. This allows components to access and use data from their parent.
- **Nested Components**: Because the context is passed down, components can be nested within each other, and the rendering will be handled recursively. For example, if `hello.html` contains `{{ component("hello.children") }}`, the `hello.children` component will be rendered with the same context available to `hello`.

- **Component Request Handling**: The framework now distinguishes between GET and POST requests to component routes.
  - **GET Requests**: Trigger the `load_template_context()` function in the component's Python file. This function is expected to return a dictionary that provides the initial context for rendering the component's template.
  - **POST Requests**: The framework looks for a hidden `<input type="hidden" name="action" value="action_name">` in the submitted form. The value of this field is used to dynamically call a corresponding `action_action_name(**kwargs)` function in the Python file. The entire form payload is passed as keyword arguments to this function. This allows components to handle form submissions and other actions.

## HttpRequest Context in Templates

The framework now provides a comprehensive `request` object within the Jinja2 templates, giving you access to essential details about the incoming HTTP request. This allows for more dynamic and context-aware templates.

### The `request` Object

The `request` object is automatically available in all page and component templates and has the following properties:

- **`request.path`**: The full URL path of the request (e.g., `/products/123`).
- **`request.method`**: The HTTP method of the request (e.g., `"GET"`, `"POST"`).
- **`request.form_data`**: A dictionary-like object containing data from a submitted form (available on POST requests).
- **`request.query_params`**: A dictionary-like object containing the URL query parameters (e.g., `?id=123`).
- **`request.path_params`**: A dictionary-like object containing dynamic segments from the URL path (e.g., the `id` in `/products/{id}`).

### How to Use

You can access these properties directly in your templates using dot notation.

#### Example: Displaying Request Information

```jinja
<p>Request Path: {{ request.path }}</p>
<p>Request Method: {{ request.method }}</p>

{% if request.query_params.search %}
  <p>You searched for: {{ request.query_params.search }}</p>
{% endif %}

{% if request.path_params.user_id %}
  <p>User ID: {{ request.path_params.user_id }}</p>
{% endif %}

{% if request.method == "POST" %}
  <h3>Form Data:</h3>
  <ul>
    {% for key, value in request.form_data %}
      <li><strong>{{ key }}:</strong> {{ value }}</li>
    {% endfor %}
  </ul>
{% endif %}
```
