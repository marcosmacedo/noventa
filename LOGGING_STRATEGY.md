# Logging Strategy

This document outlines the logging philosophy for the Noventa framework, ensuring that log messages are consistent, clear, and targeted to the right audience.

## Guiding Principles

1.  **Audience-Centric:** Every log message should be written with a specific audience in mind. Who needs to see this message, and what do they need to do with it?
2.  **Actionable:** When a log indicates a problem, it should provide clear, actionable advice on how to fix it.
3.  **Appropriate Level:** Log levels should be used to control the verbosity of the logs, allowing developers and operators to filter out noise and focus on what's important.
4.  **Configurable:** The log level should be configurable in `config.yaml` to allow users to adjust the verbosity of the application without changing the code.

## Log Audiences and Levels

We classify all log messages into three categories, each with a corresponding set of log levels:

### 1. End User

*   **Audience:** The person running the application from the command line.
*   **Purpose:** To provide clear, friendly feedback during startup and for critical, user-actionable errors.
*   **Mechanism:** Primarily `println!`, used only for messages that the user **must** see.
*   **Examples:**
    *   Confirming that a new project has been created.
    *   Reporting a missing or invalid `config.yaml` file.
    *   Informing the user that their `secret_key` is insecure.

### 2. Developer

*   **Audience:** A developer working on the application.
*   **Purpose:** To help trace the application's flow during development, especially for features like live-reloading.
*   **Mechanism:** `log::debug!` and `log::trace!`.
    *   `log::debug!`: For general development messages, such as file watcher events and component rescans. This is the default level for development mode.
    *   `log::trace!`: For highly verbose, low-level messages that are only needed for deep debugging, such as tracing individual requests through the actor system.
*   **Examples:**
    *   `log::debug!("File change detected. Reloading routes...");`
    *   `log::debug!("Found {} components.", components.len());`
    *   `log::trace!("Interpreter {} processing request for component '{}'", ...);`

### 3. Troubleshooting

*   **Audience:** A developer or system operator running the application in a production or staging environment.
*   **Purpose:** To highlight problems that need attention, from potential issues to critical errors.
*   **Mechanism:** `log::warn!` and `log::error!`.
    *   `log::warn!`: For potential issues that don't immediately break functionality but should be addressed, such as security warnings or high-latency alerts.
    *   `log::error!`: For definite errors that have caused an operation to fail, such as a template rendering error or a database connection failure.
*   **Examples:**
    *   `log::warn!("Security warning: No session key found in config.yaml...");`
    *   `log::error!("A page template failed to render: {}...", e);`

## Configuration

The log level can be configured in `config.yaml` using the `log_level` key. The available log levels are `error`, `warn`, `info`, `debug`, and `trace`.

```yaml
# config.yaml
log_level: debug
```

If `log_level` is not set, it will default to `debug` in development mode (`noventa dev`) and `info` in production mode (`noventa serve`).

## How to Write a Good Log Message

1.  **Identify the Audience:** Who are you writing this for?
2.  **Choose the Right Level:** Based on the audience and severity, pick the appropriate log level.
3.  **Be Clear and Concise:** Write a message that is easy to understand.
4.  **Provide Context:** Include relevant information, such as file paths, component names, or error details.
5.  **Make it Actionable:** If it's an error or warning, explain what the user can do to fix it.

By following this strategy, we can create a logging system that is not just a stream of text, but a powerful tool for building, debugging, and maintaining the framework.