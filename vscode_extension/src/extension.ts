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

        socket.on('close', () => {
          outputChannel.appendLine('Connection to server closed. Attempting to reconnect...');
          if (client) {
            client.stop().then(() => {
              activate(context);
            });
          } else {
            activate(context);
          }
        });
      };

      connectToServer();
    });
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: 'file', language: 'python' },
      { scheme: 'file', language: 'html' },
    ],
    outputChannel,
    traceOutputChannel: outputChannel,
    middleware: {
      handleDiagnostics: (uri, diagnostics, next) => {
        outputChannel.appendLine(`Received diagnostics for ${uri.toString()}:`);
        for (const diagnostic of diagnostics) {
          outputChannel.appendLine(`  - [${diagnostic.severity}] ${diagnostic.message}`);
          const anyDiagnostic = diagnostic as any;
          if (anyDiagnostic.data) {
            outputChannel.appendLine(`    Full error: ${JSON.stringify(anyDiagnostic.data, null, 2)}`);
          }
        }
        next(uri, diagnostics);
      },
    },
  };

  client = new LanguageClient(
    'noventaExtension',
    'Noventa Extension',
    serverOptions,
    clientOptions
  );

  await client.start();
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}