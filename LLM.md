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


## Arbitrary Parameters in Components

You can now pass arbitrary key-value parameters to components directly from your templates. These parameters are automatically passed to the component's Python logic, allowing for more dynamic and reusable components.

### How to Use

To pass parameters to a component, simply add them as keyword arguments to the `component()` function in your template.

#### Example: Passing Parameters to a `user_profile` Component

```jinja
{{ component("user_profile", user_id=123, show_email=true) }}
```

### Accessing Parameters in Python

The parameters are passed as keyword arguments to the `load_template_context` function in your component's Python file. You can access them using `**kwargs`.

#### Example: `user_profile_logic.py`

```python
def load_template_context(request, **kwargs):
    user_id = kwargs.get("user_id")
    show_email = kwargs.get("show_email", False)

    # Fetch user data based on user_id
    user = get_user_by_id(user_id)

    return {
        "user": user,
        "show_email": show_email,
        "kwargs": kwargs  # You can also pass the whole kwargs dict
    }
```

## Database Integration with SQLAlchemy and Alembic

The framework is integrated with SQLAlchemy for database interactions and uses Alembic for managing database schema migrations. This provides a robust, CLI-driven workflow for keeping your database synchronized with your models.

### Core Components

- **SQLAlchemy**: Used for defining database models in Python (`_models.py` files) and for interacting with the database within component logic.
- **Alembic**: The tool used to manage database schema migrations. It allows you to generate and apply changes to the database schema as your models evolve.
- **`framework/config.yaml`**: The central configuration file where the database connection URL is defined.
- **`database/`**: A dedicated directory containing all Alembic-related files, including the `alembic.ini` configuration and the `versions/` directory for migration scripts.

### How It Works

1.  **Centralized Configuration**: The primary database connection URL is set in `framework/config.yaml` under the `database` key.
2.  **Dynamic Discovery**: The Alembic environment (`database/env.py`) is configured to automatically discover all files ending in `_models.py` within the `web/components/` directory. It aggregates the metadata from all discovered SQLAlchemy models, ensuring that all your tables are known to Alembic.
3.  **Configuration Loading**: When you run Alembic commands, the `env.py` script reads the `framework/config.yaml` file to get the database URL, ensuring that Alembic always connects to the correct database.
4.  **Dependency Injection**: At runtime, the Rust backend reads the same `config.yaml`, initializes a SQLAlchemy session, and injects it as a `db` keyword argument into your Python component functions (`load_template_context`, `action_*`, etc.).

### Database Migration Workflow

Migrations are managed manually via the Alembic command-line interface. This provides full control over the migration process.

**Important**: All `alembic` commands must be run from the **root of the project directory**.

#### 1. Creating a New Migration

After you create a new model or modify an existing one, you must generate a new migration script.

```bash
alembic -c database/alembic.ini revision --autogenerate -m "A descriptive message about your changes"
```

**Example**:
```bash
alembic -c database/alembic.ini revision --autogenerate -m "Add user profile table"
```
This command will inspect your models, compare them to the current state of the database, and generate a new script in the `database/versions/` directory.

#### 2. Applying Migrations

To apply all pending migrations and update the database to the latest version, use the `upgrade` command:

```bash
alembic -c database/alembic.ini upgrade head
```

#### 3. Downgrading Migrations

To revert the most recent migration, use the `downgrade` command:

```bash
alembic -c database/alembic.ini downgrade -1
```

## Project Scaffolding with `noventa new`

The framework includes a command-line tool to quickly scaffold a new Noventa project, making it easy to get started with a clean and consistent structure.

### How It Works

- **`noventa new [project_name]`**: This command, built into the Rust CLI, automates the creation of a new project.
- **Cookiecutter**: Under the hood, it uses Cookiecutter to generate the project from a predefined template located in `framework/starter`.
- **Template Variables**: The `project_name` you provide is passed to the Cookiecutter template, personalizing the new project's files and directories.

### Generated Project Structure

Running `noventa new my-app` will create a new directory named `my-app` with the following structure:

```
my-app/
├── components/         # For reusable UI components
├── pages/              # For your application's pages
│   └── index.html      # A default landing page
├── database/           # Alembic database migration setup
│   ├── versions/
│   ├── alembic.ini
│   └── env.py
└── config.yaml         # Project-specific configuration
```

### How to Use

1.  **Navigate to your workspace**: Open your terminal in the directory where you want to create your new project.
2.  **Run the command**:
    ```bash
    cargo run -- new your-project-name
    ```
3.  **Start developing**: A new directory `your-project-name` will be created. You can now `cd` into it and start the development server.


## Configurable Static File Serving

The framework supports serving static files (e.g., CSS, JavaScript, images) from a configurable directory and URL. This feature is optional and can be enabled via `config.yaml`.

### How It Works

- **`actix-files`**: This crate provides a service for serving files from a specified directory.
- **Configuration**: The static file serving is controlled by two keys in `web/config.yaml`:
  - **`static_path`**: The path to the directory containing your static files. This can be a relative path (from the project root) or an absolute path. If this key is not present, static file serving is disabled.
  - **`static_url_prefix`**: The URL prefix where the static files will be served. If this key is not present, it defaults to `/static`.
- **File Listing**: For development purposes, directory listing is enabled, so you can navigate to the configured URL prefix in your browser to see a list of all available files.

### How to Use

1.  **Enable in `config.yaml`**: To enable static file serving, add or uncomment the `static_path` and optionally the `static_url_prefix` in `web/config.yaml`:
    ```yaml
    # Path to the static files directory. Can be relative or absolute.
    static_path: "files"

    # The URL prefix for static files. Defaults to "/static" if not specified.
    static_url_prefix: "/files"
    ```
2.  **Place your static files**: Create the directory specified in `static_path` (e.g., `web/files`) and place your assets inside it.
3.  **Link to them in your templates**: In your HTML templates, you can now link to these files using the configured `static_url_prefix`.

    **Example**:
    ```html
    <link rel="stylesheet" href="/files/style.css">
    <script src="/files/main.js"></script>
    <img src="/files/logo.png" alt="Logo">
    ```

## File System Tools

The framework provides a suite of interactive tools for managing files and directories within your project. These tools are designed with contextual awareness of the Noventa project structure, providing helpful feedback and warnings to ensure that files are created in the correct locations.

All file system tools include a critical security check that prevents any operations outside the current working directory, ensuring the safety and integrity of your system.

### `read_file`

Reads the contents of a file and provides contextual metadata based on its location and naming convention.

- **Component Files**: Identifies component logic, templates, and models and indicates which component they belong to.
- **Page Templates**: For `.html` files in the `web/pages` directory, it displays the browser route that the page will generate.
- **Layouts**: Identifies Jinja2 layouts in the `web/layouts` directory.

### `list_directory`

Lists the contents of a directory in a clean, tabular format with `Path` and `Type` columns.

- **Relative Paths**: All paths are displayed relative to the queried directory.
- **Contextual Types**: The `Type` column provides specific information for Noventa file types (e.g., "Component Logic", "Page Template", "Layout").

### `create_directory`

Creates a new directory and provides a descriptive success message that indicates where the directory was created, with special contextual information for Noventa's special directories.

### `write_file`

Writes content to a file, automatically creating any necessary parent directories. This tool is context-aware and provides helpful warnings to enforce the project's structure.

- **Automatic Directory Creation**: If any parent directories in the path do not exist, they will be created automatically.
- **Contextual Warnings**:
    - **Component Files**: If you write a component file (e.g., `_logic.py`) outside the `web/components` directory, it will issue a warning and suggest the correct location. It also validates that component files are placed within a subdirectory of `web/components` and follow the correct naming conventions.
    - **HTML Files**: If you write an `.html` file outside of the `web/pages` or `web/layouts` directories, it will provide a warning suggesting the correct placement for pages or layouts.

### `delete_directory`

Recursively deletes a directory and its contents. This tool includes important safeguards to prevent accidental deletion of critical project directories.

- **Protected Directories**: It is impossible to delete the `web/components`, `web/pages`, or `web/layouts` directories.
- **Security**: This tool cannot delete directories outside the current working directory.

### `delete_file`

Deletes a single file.

- **Security**: This tool cannot delete files outside the current working directory.
