import * as child_process from 'child_process';
import { existsSync } from 'fs';
import * as net from 'net';
import * as path from 'path';
import * as vscode from 'vscode';
import {
  ExecuteCommandRequest,
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  StreamInfo,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;
let output: vscode.OutputChannel;
let statusBar: vscode.StatusBarItem;
let signaturesEnabled = true;

const CONFIG_SECTION = 'tyda';

// The tyda LSP server advertises these workspace commands (see src/lsp.rs).
// `createSignature` is the command a code lens carries: clicking the inferred
// signature lens asks the server to insert it as a `#:` comment. enable/disable
// toggle the inline signature lenses. The server does the work (it applies the
// edit via workspace/applyEdit); the client only forwards the request.
const CMD_CREATE_SIGNATURE = 'typeprof.createSignature';
const CMD_ENABLE_SIGNATURE = 'typeprof.enableSignature';
const CMD_DISABLE_SIGNATURE = 'typeprof.disableSignature';
const CMD_TOGGLE = 'tyda.toggleSignature';

// Resolve the tyda binary: an explicit `tyda.server.path` wins (handy for local
// development against `target/release/tyda`); otherwise prefer the binary
// bundled into the extension (`bin/tyda`, staged per-platform by the release
// pipeline); finally fall back to `tyda` on PATH.
function resolveBinary(context: vscode.ExtensionContext): string {
  const configured = vscode.workspace
    .getConfiguration(CONFIG_SECTION)
    .get<string | null>('server.path');
  if (configured && configured.trim().length > 0) {
    return configured.trim();
  }
  const name = process.platform === 'win32' ? 'tyda.exe' : 'tyda';
  const bundled = path.join(context.extensionPath, 'bin', name);
  if (existsSync(bundled)) {
    return bundled;
  }
  return name; // PATH
}

// tyda's `--lsp` prints a single JSON line `{host, port, pid}` to stdout and then
// serves LSP over TCP (TypeProf-compatible). Spawn it, wait for that line, and
// connect a socket the LanguageClient reads/writes.
function startServer(context: vscode.ExtensionContext, cwd: string | undefined): Promise<StreamInfo> {
  return new Promise<StreamInfo>((resolve, reject) => {
    const bin = resolveBinary(context);
    const config = vscode.workspace.getConfiguration(CONFIG_SECTION);

    const env = { ...process.env };
    if (config.get<boolean>('experimentalChecks')) {
      env.TYDA_EXPERIMENTAL_CHECKS = '1';
    }

    output.appendLine(`[tyda] starting language server: ${bin} --lsp`);
    const server = child_process.spawn(bin, ['--lsp'], { cwd, env });

    let buffer = '';
    let connected = false;
    server.stdout.on('data', (data: Buffer) => {
      if (connected) {
        return;
      }
      buffer += data.toString();
      try {
        const info = JSON.parse(buffer) as { host: string; port: number; pid: number };
        connected = true;
        const socket = net.connect(info.port, info.host, () => {
          resolve({ reader: socket, writer: socket });
        });
        socket.on('error', reject);
      } catch {
        // startup JSON not fully buffered yet — keep reading.
      }
    });
    server.stderr.on('data', (data: Buffer) => output.append(data.toString()));
    server.on('error', (err) => {
      output.appendLine(`[tyda] failed to start: ${err}`);
      reject(err);
    });
    server.on('exit', (code) => {
      if (!connected) {
        reject(new Error(`tyda --lsp exited with code ${code} before announcing a port`));
      }
    });
  });
}

// Run a server-side workspace command. Code-lens clicks invoke the command id
// directly, so we register a handler that relays it (with its arguments) to the
// server, which performs the edit.
async function runServerCommand(command: string, args: unknown[]): Promise<void> {
  if (!client) {
    return;
  }
  try {
    await client.sendRequest(ExecuteCommandRequest.type, { command, arguments: args });
  } catch (err) {
    output.appendLine(`[tyda] command ${command} failed: ${err}`);
  }
}

function updateStatusBar(): void {
  statusBar.text = signaturesEnabled ? '$(symbol-type) Tyda' : '$(eye-closed) Tyda';
  statusBar.tooltip = signaturesEnabled
    ? 'Tyda inline signatures: on (click to hide)'
    : 'Tyda inline signatures: off (click to show)';
  statusBar.command = CMD_TOGGLE;
  statusBar.show();
}

export function activate(context: vscode.ExtensionContext): void {
  output = vscode.window.createOutputChannel('Tyda');
  context.subscriptions.push(output);

  const folder = vscode.workspace.workspaceFolders?.[0];
  const cwd = folder?.uri.fsPath;

  const serverOptions: ServerOptions = () => startServer(context, cwd);
  const clientOptions: LanguageClientOptions = {
    // Hover, code lens, diagnostics, completion, and go-to-definition are all
    // negotiated automatically from the server's capabilities for this selector.
    documentSelector: [{ scheme: 'file', language: 'ruby' }],
    outputChannel: output,
  };

  client = new LanguageClient('tyda', 'Tyda', serverOptions, clientOptions);
  client.start();

  // Relay the code-lens command to the server.
  context.subscriptions.push(
    vscode.commands.registerCommand(CMD_CREATE_SIGNATURE, (...args: unknown[]) =>
      runServerCommand(CMD_CREATE_SIGNATURE, args),
    ),
  );

  // Toggle inline signatures (status bar + command palette).
  statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
  context.subscriptions.push(
    statusBar,
    vscode.commands.registerCommand(CMD_TOGGLE, async () => {
      signaturesEnabled = !signaturesEnabled;
      await runServerCommand(
        signaturesEnabled ? CMD_ENABLE_SIGNATURE : CMD_DISABLE_SIGNATURE,
        [],
      );
      updateStatusBar();
    }),
  );
  updateStatusBar();

  // Restart the server when the binary path or experimental flag changes.
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (
        event.affectsConfiguration(`${CONFIG_SECTION}.server.path`) ||
        event.affectsConfiguration(`${CONFIG_SECTION}.experimentalChecks`)
      ) {
        void client?.restart();
      }
    }),
  );
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
