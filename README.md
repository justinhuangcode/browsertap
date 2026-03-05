# browsertap

**English** | [中文](./README_CN.md)

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org)
[![TypeScript](https://img.shields.io/badge/typescript-5.7%2B-blue.svg?style=flat-square&logo=typescript&logoColor=white)](runtime/browser/)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey?style=flat-square)](https://github.com/justinhuangcode/browsertap)
[![GitHub Stars](https://img.shields.io/github/stars/justinhuangcode/browsertap?style=flat-square&logo=github)](https://github.com/justinhuangcode/browsertap/stargazers)
[![Last Commit](https://img.shields.io/github/last-commit/justinhuangcode/browsertap?style=flat-square)](https://github.com/justinhuangcode/browsertap/commits/main)
[![Issues](https://img.shields.io/github/issues/justinhuangcode/browsertap?style=flat-square)](https://github.com/justinhuangcode/browsertap/issues)

Tap into your live browser. Close the agent loop. Built in Rust.

browsertap lets AI agents and CLI tools control an **already-open, already-authenticated** browser session -- screenshots, JS execution, smoke tests, console capture, and more -- without spinning up headless instances or re-logging in.

## Why browsertap?

AI agents that interact with web apps need to **see and control the real thing**. They need to run JS, take screenshots, check console errors, and click buttons -- all in a browser that's already logged in with real cookies, real sessions, and real state.

Existing tools don't fit this workflow:

| | browsertap | Playwright | Puppeteer |
|---|---|---|---|
| Attaches to live browser tab | **Yes** | No (new instance) | No (new instance) |
| Preserves auth state | **Yes** | No (re-login) | No (re-login) |
| Runtime dependency | **None** (single binary) | Node.js | Node.js |
| Binary size | **~5 MB** | ~100 MB+ | ~100 MB+ |
| Startup time | **< 10 ms** | > 500 ms | > 500 ms |
| Built-in smoke testing | **Yes** (parallel) | No | No |
| Session codenames | **Yes** | No | No |
| Console/network buffering | **Yes** | Via code only | Via code only |
| Self-signed TLS | **Built-in** (rcgen + rustls) | N/A | N/A |
| Designed for AI agents | **Yes** | No (test framework) | No (library) |

**The typical AI agent workflow with browsertap:**

```
Developer has web app open in browser (already logged in)
        |
        v
@browsertap/runtime connects the tab to the daemon
        |
        v
AI agent runs: browsertap run-js iron-falcon "document.title"
        |
        v
AI agent runs: browsertap screenshot iron-falcon -o page.jpg
        |
        v
AI agent inspects the screenshot / queries DOM / checks console
        |
        v
AI agent runs: browsertap smoke iron-falcon --preset main
        |
        v
No headless browser. No re-login. No lost state.
```

## Features

- **Attach to live sessions** -- Control an already-open, already-authenticated browser tab
- **Daemon architecture** -- `browsertapd` runs as a persistent HTTPS + WebSocket hub; CLI commands talk to it via REST API
- **Session codenames** -- Friendly names like `iron-falcon` or `calm-otter` instead of UUIDs
- **JavaScript execution** -- Run arbitrary JS in the browser context via CLI
- **Screenshot capture** -- Full page or element-specific via CSS selector
- **Console capture** -- View browser console output with level filtering; buffer survives CLI reconnect
- **Network capture** -- Inspect HTTP requests/responses buffered by the runtime
- **Smoke testing** -- Automated route sweep with presets, error detection, and progress tracking
- **Selector discovery** -- Find interactive elements on the page (buttons, links, inputs)
- **HMAC-SHA256 tokens** -- Short-lived session tokens (5 min) and CLI tokens (1 hour)
- **Self-signed TLS** -- Auto-generated certificates via rcgen + rustls, zero external tools
- **Auto-reconnect** -- Browser runtime reconnects with exponential backoff after disconnects
- **Config file walk-up** -- Place `browsertap.toml` at project root; CLI finds it automatically
- **JSON output** -- Machine-readable output for agent integration
- **Cross-platform** -- macOS, Linux, and Windows

## Installation

### Pre-built binaries (coming soon)

Pre-built binaries for all platforms will be available on [GitHub Releases](https://github.com/justinhuangcode/browsertap/releases).

| Platform | Binary |
|---|---|
| Linux x86_64 | `browsertap-v*-linux-x86_64.tar.gz` |
| Linux ARM64 | `browsertap-v*-linux-arm64.tar.gz` |
| macOS Intel | `browsertap-v*-macos-x86_64.tar.gz` |
| macOS Apple Silicon | `browsertap-v*-macos-arm64.tar.gz` |
| Windows x86_64 | `browsertap-v*-windows-x86_64.zip` |

### Via Cargo

```bash
# Install both binaries
cargo install --path crates/cli
cargo install --path crates/daemon
```

### Browser runtime SDK

```bash
npm install @browsertap/runtime
```

### From source

```bash
git clone https://github.com/justinhuangcode/browsertap.git
cd browsertap
cargo build --release
# Binaries at: target/release/browsertap, target/release/browsertapd
```

**Requirements:** Rust 1.75+ and a Chromium-based browser for the page you want to control.

## Quick Start

### 1. Start the daemon

```bash
browsertapd
# => browsertapd listening on https://127.0.0.1:4455
```

### 2. Integrate the browser runtime into your web app

```typescript
import { createBrowserTapClient, createSessionStorageAdapter } from '@browsertap/runtime';

const client = createBrowserTapClient({
  storage: createSessionStorageAdapter(),
  onStatus: (snap) => console.log('browsertap:', snap.status, snap.codename),
  autoReconnectHandshake: () =>
    fetch('/api/browsertap/handshake', { method: 'POST' }).then(r => r.json()),
});

const handshake = await fetch('/api/browsertap/handshake', { method: 'POST' }).then(r => r.json());
await client.startSession(handshake);
// => "connected as iron-falcon"
```

### 3. Control from CLI

```bash
browsertap sessions
# CODENAME             URL                                      STATE      HEARTBEAT
# iron-falcon          http://localhost:3000/dashboard           open       2s ago

browsertap run-js iron-falcon "document.title"
# "Dashboard - MyApp"

browsertap screenshot iron-falcon --selector "#analytics" -o card.jpg
# Screenshot saved to card.jpg (45832 bytes)
```

## Commands

| Command | Description |
|---|---|
| `daemon` | Start the browsertap daemon (delegates to `browsertapd`) |
| `sessions` | List active browser sessions with codenames and heartbeat status |
| `run-js <session> <code>` | Execute JavaScript in a browser session |
| `screenshot <session>` | Capture page or element screenshot |
| `click <session> <selector>` | Click an element by CSS selector |
| `navigate <session> <url>` | Navigate a session to a URL |
| `smoke <session>` | Run smoke tests across configured routes |
| `console <session>` | View console logs from a session |
| `selectors <session>` | Discover interactive selectors on the page |

## Command Flags

### Global Flags

| Flag | Default | Description |
|---|---|---|
| `--daemon-url <url>` | `https://127.0.0.1:4455` | Daemon URL (also via `BROWSERTAP_DAEMON_URL`) |

### `screenshot` Flags

| Flag | Default | Description |
|---|---|---|
| `-s, --selector <sel>` | *(full page)* | CSS selector of element to capture |
| `-o, --output <path>` | `screenshot.jpg` | Output file path |
| `--quality <f32>` | `0.85` | JPEG quality (0.0 - 1.0) |

### `smoke` Flags

| Flag | Default | Description |
|---|---|---|
| `--preset <name>` | `defaults` | Route preset name from `browsertap.toml` |
| `--routes <list>` | *(none)* | Comma-separated route list |
| `--parallel <n>` | `1` | Number of parallel workers |

### `console` Flags

| Flag | Default | Description |
|---|---|---|
| `-t, --tail <n>` | `50` | Number of recent events to show |
| `--level <level>` | *(all)* | Filter by level: log, info, warn, error |

## How It Works

1. **`browsertapd`** starts an HTTPS + WebSocket server on `127.0.0.1:4455`. It auto-generates self-signed TLS certificates on first run and stores them at `~/.browsertap/certs/`.

2. **Your web app** embeds `@browsertap/runtime`. When activated, the runtime calls your backend's handshake endpoint, which mints an HMAC-SHA256 signed session token using the shared secret.

3. **The browser runtime** opens a WebSocket to the daemon, sends a `register` message with the signed token, and receives a friendly codename (e.g., `iron-falcon`). It then patches `console.*` to capture logs and starts a heartbeat every 5 seconds.

4. **CLI commands** (`browsertap run-js iron-falcon "..."`) send HTTPS requests to the daemon's REST API. The daemon forwards the command to the browser via WebSocket, waits for the result, and returns it to the CLI.

5. **Console and network events** are buffered in the daemon (500 console events, 200 network events per session). The CLI can retroactively query these buffers, even for events that occurred before the CLI connected.

## Architecture

```
                            WebSocket (wss://)
+------------------+                                +------------------+
|  Your Web App    |  ------>  +--------------+     |  CLI / AI Agent  |
|  (logged in)     |  <------  |  browsertapd |     |                  |
|                  |           |              |     |  $ browsertap    |
| @browsertap/     |  register |  Session     | <-- |    run-js        |
|   runtime        |  heartbeat|  Registry    | --> |    screenshot    |
|                  |  console  |  Command     |     |    smoke         |
|                  |  result   |  Router      |     |    console       |
+------------------+           |  TLS (rustls)|     +------------------+
                               +--------------+
                                 HTTPS REST API
```

## Configuration

Create `browsertap.toml` at your project root. The CLI walks up directories to find it.

```toml
app_label = "MyApp"
app_url = "http://localhost:3000"
daemon_url = "https://127.0.0.1:4455"

[daemon]
host = "127.0.0.1"
port = 4455

[smoke]
defaults = ["dashboard", "settings", "profile"]

[smoke.presets]
main = ["dashboard", "settings", "profile", "billing"]
quick = ["dashboard"]

[smoke.redirects]
"/" = "/dashboard"
```

**Resolution order:** CLI flags > Environment variables > `browsertap.toml` > Defaults

### Environment Variables

| Variable | Description |
|---|---|
| `BROWSERTAP_DAEMON_URL` | Daemon URL |
| `BROWSERTAP_HOST` | Daemon listen host |
| `BROWSERTAP_PORT` | Daemon listen port |
| `BROWSERTAP_SECRET` | Shared secret (hex string) |

## Backend Handshake Endpoint

Your web app backend needs one endpoint to mint session tokens:

```typescript
// POST /api/browsertap/handshake
import { readFileSync } from 'fs';
import { createHmac, randomUUID } from 'crypto';

export async function POST() {
  const secret = process.env.BROWSERTAP_SECRET
    ?? readFileSync(`${process.env.HOME}/.browsertap/secret.key`, 'utf8').trim();

  const sessionId = randomUUID();
  const payload = {
    token_id: randomUUID(),
    scope: 'session',
    subject: 'browsertap-web',
    session_id: sessionId,
    issued_at: new Date().toISOString(),
    expires_at: new Date(Date.now() + 5 * 60 * 1000).toISOString(),
  };

  const encoded = Buffer.from(JSON.stringify(payload)).toString('base64url');
  const sig = createHmac('sha256', Buffer.from(secret, 'hex'))
    .update(encoded).digest('base64url');

  return Response.json({
    sessionId,
    sessionToken: `${encoded}.${sig}`,
    socketUrl: 'wss://127.0.0.1:4455/bridge',
    expiresAt: Math.floor(Date.now() / 1000) + 300,
  });
}
```

## Security & Threat Model

browsertap is designed for **single-user, local-only** use on development machines.

| Layer | Control | Detail |
|---|---|---|
| **HTTPS server** | Localhost-only | Binds to `127.0.0.1`; never exposed to the network |
| **TLS** | Auto-generated certs | Self-signed via rcgen + rustls at `~/.browsertap/certs/` |
| **Session tokens** | HMAC-SHA256, short-lived | Browser tokens expire in 5 minutes; CLI tokens in 1 hour |
| **Token verification** | Constant-time | Uses `hmac` crate's timing-safe comparison |
| **Secret storage** | Owner-only permissions | `~/.browsertap/secret.key` created with mode `0600` (Unix) |
| **Console buffer** | Bounded | Max 500 events per session to prevent memory exhaustion |

### Not recommended for

- **Multi-user / shared machines** -- Other local users with root access can read the session token
- **Production workloads** -- browsertap is a development/testing tool; no rate limiting or audit logging
- **Untrusted networks** -- Self-signed certificates are not verified by default

## Project Structure

```
browsertap/
├── Cargo.toml                    # Workspace root
├── browsertap.toml               # Example project config
├── crates/
│   ├── shared/                   # Shared library (tokens, protocol, types)
│   │   └── src/
│   │       ├── lib.rs            # Module exports
│   │       ├── token.rs          # HMAC-SHA256 token sign/verify
│   │       ├── protocol.rs       # WebSocket + REST protocol types
│   │       ├── session.rs        # Session state, config types
│   │       └── codename.rs       # Friendly codename generation
│   ├── daemon/                   # Daemon binary (browsertapd)
│   │   └── src/
│   │       ├── main.rs           # Axum HTTPS server + REST routes
│   │       ├── state.rs          # Session registry, command routing
│   │       ├── websocket.rs      # WebSocket handler (register/heartbeat/command)
│   │       └── tls.rs            # Self-signed cert generation (rcgen)
│   └── cli/                      # CLI binary (browsertap)
│       └── src/
│           ├── main.rs           # Clap command definitions
│           ├── client.rs         # HTTP client for daemon REST API
│           └── config.rs         # browsertap.toml loader (walk-up)
└── runtime/
    └── browser/                  # Browser runtime SDK (TypeScript)
        ├── package.json          # @browsertap/runtime
        ├── tsconfig.json
        └── src/
            ├── index.ts          # Public API exports
            ├── client.ts         # WebSocket lifecycle, command executor, console patch
            ├── types.ts          # TypeScript type definitions
            └── storage.ts        # Session persistence adapters
```

## Roadmap

- [ ] Cookie sync from main Chrome profile
- [ ] Built-in OAuth automation (GitHub, Google, Twitter)
- [ ] Parallel smoke testing
- [ ] Visual regression (screenshot diff)
- [ ] Network request interception
- [ ] State snapshots (save/restore cookies + localStorage)
- [ ] Real-time event streaming (SSE)
- [ ] OS Keychain integration
- [ ] WebDriver BiDi support (Firefox)
- [ ] WASM plugin system
- [ ] CI/CD pipeline (GitHub Actions)
- [ ] Pre-built binary releases
- [ ] Homebrew tap

## Contributing

Contributions are welcome! Please open an issue to discuss your idea before submitting a PR.

## Changelog

See [Releases](https://github.com/justinhuangcode/browsertap/releases) for version history.

## License

[MIT](LICENSE)
