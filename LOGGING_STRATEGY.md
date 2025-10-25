# Logging and Error Reporting Strategy

This document outlines the logging and error reporting strategy for the Noventa framework, including the new LSP-based error reporting mechanism.

## LSP-Based Error Reporting

The framework now includes an integrated Language Server Protocol (LSP) server that provides real-time error reporting directly in the VSCode editor. This feature is designed to improve the development experience by providing immediate feedback on errors as they occur.

### How it Works

1.  **LSP Server:** The LSP server is implemented as an `actix` actor and runs on a TCP socket (default port: 9999) when the application is started in development mode (`noventa dev`).
2.  **Error Broadcasting:** When an error occurs in the application, it is sent to a global `tokio::sync::broadcast` channel.
3.  **LSP Backend:** The LSP server's `Backend` subscribes to this channel and listens for incoming errors.
4.  **Diagnostics:** When an error is received, the `Backend` parses it, creates a `Diagnostic` object, and publishes it to the VSCode client.

### Configuration

The VSCode extension includes the following configuration options:

-   `lspDiagnosticsForwarder.enable`: Enable or disable the LSP client.
-   `lspDiagnosticsForwarder.port`: The port to connect to the LSP server on.

## Standard Logging

In addition to the LSP-based error reporting, the framework also uses the `log` crate for standard logging. The log level can be configured in the `config.yaml` file.

### How to Run the Extension in Development Mode

To test and develop the extension locally, follow these steps:

1.  **Navigate to the Extension Directory:**
    Open a terminal and change into the extension's directory:
    ```bash
    cd vscode-lsp-diagnostics-forwarder
    ```

2.  **Install Dependencies:**
    If you haven't already, install the necessary `npm` packages:
    ```bash
    npm install
    ```

3.  **Compile the Extension:**
    The extension is written in TypeScript and needs to be compiled into JavaScript.
    ```bash
    npm run compile
    ```
    You can also run `npm run watch` to automatically recompile the extension whenever you make changes to the source code.

4.  **Launch the Extension Development Host:**
    Open the `vscode-lsp-diagnostics-forwarder` folder in VSCode. Then, press `F5` (or go to `Run > Start Debugging`). This will open a new VSCode window, called the "Extension Development Host," with your extension running inside it.

5.  **Start the Noventa Server:**
    In a separate terminal, navigate to the `framework` directory and start the Noventa development server:
    ```bash
    cd ../framework
    noventa dev
    ```
    This will start both the web server and the LSP server (on port 9999 by default).

6.  **Verify the Connection:**
    In the Extension Development Host window, open a file and trigger an error in your Noventa application. You should see diagnostics appear in the editor. You can also open the "Output" panel and select "LSP Diagnostics Forwarder" from the dropdown to see the raw LSP messages being exchanged between the client and the server.