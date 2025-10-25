"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const net = require("net");
const vscode = require("vscode");
const node_1 = require("vscode-languageclient/node");
let client;
async function activate(context) {
    const config = vscode.workspace.getConfiguration('lspDiagnosticsForwarder');
    const enabled = config.get('enable', true);
    const port = config.get('port', 9999);
    if (!enabled) {
        return;
    }
    const outputChannel = vscode.window.createOutputChannel('LSP Diagnostics Forwarder');
    const serverOptions = () => {
        return new Promise((resolve) => {
            const connectToServer = () => {
                outputChannel.appendLine(`Attempting to connect to LSP server on port ${port}...`);
                const socket = net.connect({ port });
                socket.on('connect', () => {
                    outputChannel.appendLine('Successfully connected to LSP server.');
                    resolve({
                        reader: socket,
                        writer: socket
                    });
                });
                socket.on('error', (err) => {
                    outputChannel.appendLine(`Connection failed: ${err.message}. Retrying in 5 seconds...`);
                    setTimeout(connectToServer, 5000);
                });
            };
            connectToServer();
        });
    };
    const clientOptions = {
        documentSelector: [{ scheme: 'file', language: 'plaintext' }],
        outputChannel,
        traceOutputChannel: outputChannel,
    };
    client = new node_1.LanguageClient('lspDiagnosticsForwarder', 'LSP Diagnostics Forwarder', serverOptions, clientOptions);
    await client.start();
    client.onNotification('textDocument/publishDiagnostics', (params) => {
        outputChannel.appendLine(`Received diagnostics for ${params.uri}:`);
        for (const diagnostic of params.diagnostics) {
            outputChannel.appendLine(`  - [${diagnostic.severity}] ${diagnostic.message}`);
            if (diagnostic.data) {
                outputChannel.appendLine(`    Full error: ${JSON.stringify(diagnostic.data, null, 2)}`);
            }
        }
    });
}
function deactivate() {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
//# sourceMappingURL=extension.js.map