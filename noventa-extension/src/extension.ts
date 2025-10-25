import * as net from 'net';
import * as vscode from 'vscode';
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  StreamInfo
} from 'vscode-languageclient/node';

let client: LanguageClient;

export async function activate(context: vscode.ExtensionContext) {
  const config = vscode.workspace.getConfiguration('noventaExtension');
  const enabled = config.get('enable', true);
  const port = config.get('port', 9090);

  if (!enabled) {
    return;
  }

  const outputChannel = vscode.window.createOutputChannel('Noventa Extension');

  const serverOptions: ServerOptions = () => {
    return new Promise((resolve) => {
      const connectToServer = () => {
        outputChannel.appendLine(`Attempting to connect to Noventa server on port ${port}...`);
        const socket = net.connect({ port });

        socket.on('connect', () => {
          outputChannel.appendLine('Successfully connected to Noventa server.');
          resolve({
            reader: socket,
            writer: socket
          } as StreamInfo);
        });

        socket.on('error', (err) => {
          outputChannel.appendLine(`Connection failed: ${err.message}. Retrying in 5 seconds...`);
          setTimeout(connectToServer, 5000);
        });
      };

      connectToServer();
    });
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: 'file', language: 'plaintext' }],
    outputChannel,
    traceOutputChannel: outputChannel,
  };

  client = new LanguageClient(
    'noventaExtension',
    'Noventa Extension',
    serverOptions,
    clientOptions
  );

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

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}