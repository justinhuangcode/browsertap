import type {
  BrowserTapClient,
  BrowserTapOptions,
  BrowserCommand,
  CommandResult,
  ConnectionStatus,
  DaemonMessage,
  HandshakeResult,
  StatusSnapshot,
  StoredSession,
} from "./types";

const HEARTBEAT_INTERVAL_MS = 5_000;
const CONSOLE_FLUSH_INTERVAL_MS = 500;
const MAX_CONSOLE_BUFFER = 200;
const RECONNECT_BASE_MS = 1_500;
const RECONNECT_MAX_MS = 15_000;

interface ConsoleEvent {
  id: string;
  timestamp: number;
  level: string;
  args: unknown[];
}

/**
 * Create a BrowserTap client that connects to the daemon via WebSocket.
 *
 * Usage:
 * ```ts
 * import { createBrowserTapClient, createSessionStorageAdapter } from '@browsertap/runtime';
 *
 * const client = createBrowserTapClient({
 *   storage: createSessionStorageAdapter(),
 *   onStatus: (snap) => console.log('browsertap:', snap.status, snap.codename),
 *   autoReconnectHandshake: () =>
 *     fetch('/api/browsertap/handshake', { method: 'POST' }).then(r => r.json()),
 * });
 *
 * // Start from a handshake result
 * const handshake = await fetch('/api/browsertap/handshake', { method: 'POST' }).then(r => r.json());
 * await client.startSession(handshake);
 * ```
 */
export function createBrowserTapClient(options: BrowserTapOptions = {}): BrowserTapClient {
  const {
    storage,
    onStatus,
    autoReconnectHandshake,
    maxReconnectAttempts = 5,
    captureConsole = true,
  } = options;

  let socket: WebSocket | null = null;
  let sessionId: string | null = null;
  let codename: string | null = null;
  let status: ConnectionStatus = "disconnected";
  let heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  let consoleFlushTimer: ReturnType<typeof setInterval> | null = null;
  let reconnectAttempts = 0;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  // Console event buffer
  const consoleBuffer: ConsoleEvent[] = [];
  let eventCounter = 0;

  // Original console methods (patched for capture)
  const originalConsole: Record<string, (...args: unknown[]) => void> = {};

  // ─── Status management ────────────────────────────────────────────

  function setStatus(newStatus: ConnectionStatus, reason?: string): void {
    status = newStatus;
    onStatus?.({
      status: newStatus,
      codename: codename ?? undefined,
      sessionId: sessionId ?? undefined,
      reason,
    });
  }

  function getStatus(): StatusSnapshot {
    return {
      status,
      codename: codename ?? undefined,
      sessionId: sessionId ?? undefined,
    };
  }

  // ─── Console patching ─────────────────────────────────────────────

  function patchConsole(): void {
    if (!captureConsole) return;

    const levels = ["log", "info", "warn", "error", "debug"] as const;
    for (const level of levels) {
      originalConsole[level] = console[level].bind(console);
      (console as unknown as Record<string, unknown>)[level] = (...args: unknown[]) => {
        // Buffer the event
        consoleBuffer.push({
          id: `evt-${++eventCounter}`,
          timestamp: Date.now(),
          level,
          args: args.map(sanitizeArg),
        });

        // Enforce buffer limit
        while (consoleBuffer.length > MAX_CONSOLE_BUFFER) {
          consoleBuffer.shift();
        }

        // Call original
        originalConsole[level](...args);
      };
    }
  }

  function unpatchConsole(): void {
    for (const [level, fn] of Object.entries(originalConsole)) {
      (console as unknown as Record<string, unknown>)[level] = fn;
    }
  }

  function sanitizeArg(arg: unknown): unknown {
    if (arg === null || arg === undefined) return arg;
    if (typeof arg === "string" || typeof arg === "number" || typeof arg === "boolean") return arg;
    try {
      // Try JSON roundtrip to strip non-serializable values
      return JSON.parse(JSON.stringify(arg));
    } catch {
      return String(arg);
    }
  }

  function flushConsoleBuffer(): void {
    if (!socket || socket.readyState !== WebSocket.OPEN || consoleBuffer.length === 0) return;

    const events = consoleBuffer.splice(0, consoleBuffer.length);
    socket.send(JSON.stringify({
      kind: "console",
      sessionId,
      events,
    }));
  }

  // ─── Command execution ────────────────────────────────────────────

  async function executeCommand(command: BrowserCommand): Promise<CommandResult> {
    const start = performance.now();
    try {
      let data: unknown;

      switch (command.type) {
        case "runScript": {
          const code = command.code as string;
          // Execute in an async wrapper to support await
          const fn = new Function(`return (async () => { return (${code}); })()`);
          data = await fn();
          break;
        }
        case "screenshot": {
          // Use html2canvas if available, otherwise return a placeholder
          data = { error: "screenshot requires html2canvas - import it in your app" };
          break;
        }
        case "click": {
          const selector = command.selector as string;
          const el = document.querySelector(selector);
          if (!el) throw new Error(`element not found: ${selector}`);
          (el as HTMLElement).click();
          data = { clicked: selector };
          break;
        }
        case "navigate": {
          const url = command.url as string;
          window.location.assign(url);
          data = { navigated: url };
          break;
        }
        case "discoverSelectors": {
          const selectors = discoverInteractiveSelectors();
          data = selectors;
          break;
        }
        default:
          throw new Error(`unknown command type: ${command.type}`);
      }

      return {
        ok: true,
        data,
        durationMs: Math.round(performance.now() - start),
      };
    } catch (err) {
      return {
        ok: false,
        error: err instanceof Error ? err.message : String(err),
        durationMs: Math.round(performance.now() - start),
      };
    }
  }

  function discoverInteractiveSelectors(): { tag: string; selector: string; text: string }[] {
    const elements = document.querySelectorAll(
      "a, button, input, select, textarea, [role='button'], [onclick], [data-testid]"
    );
    const results: { tag: string; selector: string; text: string }[] = [];

    elements.forEach((el) => {
      const tag = el.tagName.toLowerCase();
      let selector = tag;

      if (el.id) {
        selector = `#${el.id}`;
      } else if (el.getAttribute("data-testid")) {
        selector = `[data-testid="${el.getAttribute("data-testid")}"]`;
      } else if (el.className && typeof el.className === "string") {
        const cls = el.className.split(" ").filter(Boolean).slice(0, 2).join(".");
        if (cls) selector = `${tag}.${cls}`;
      }

      results.push({
        tag,
        selector,
        text: (el.textContent ?? "").trim().slice(0, 60),
      });
    });

    return results.slice(0, 100); // Limit to 100 elements
  }

  // ─── WebSocket lifecycle ──────────────────────────────────────────

  function connect(handshake: HandshakeResult): void {
    setStatus("connecting");
    sessionId = handshake.sessionId;

    socket = new WebSocket(handshake.socketUrl, ["browsertap"]);

    socket.onopen = () => {
      socket!.send(JSON.stringify({
        kind: "register",
        token: handshake.sessionToken,
        sessionId: handshake.sessionId,
        url: window.location.href,
        title: document.title,
        userAgent: navigator.userAgent,
        topOrigin: window.location.origin,
      }));

      // Start heartbeat
      heartbeatTimer = setInterval(() => {
        if (socket?.readyState === WebSocket.OPEN) {
          socket.send(JSON.stringify({ kind: "heartbeat", sessionId }));
        }
      }, HEARTBEAT_INTERVAL_MS);

      // Start console flush
      if (captureConsole) {
        consoleFlushTimer = setInterval(flushConsoleBuffer, CONSOLE_FLUSH_INTERVAL_MS);
      }

      reconnectAttempts = 0;
    };

    socket.onmessage = (event) => {
      let msg: DaemonMessage;
      try {
        msg = JSON.parse(event.data as string);
      } catch {
        return;
      }

      switch (msg.kind) {
        case "metadata": {
          codename = msg.codename as string;
          setStatus("connected");

          // Persist session for reconnect
          storage?.set({
            sessionId: handshake.sessionId,
            sessionToken: handshake.sessionToken,
            socketUrl: handshake.socketUrl,
            expiresAtMs: handshake.expiresAt * 1000,
            codename: codename ?? undefined,
          });
          break;
        }
        case "command": {
          const command = msg.command as BrowserCommand;
          executeCommand(command).then((result) => {
            socket?.send(JSON.stringify({
              kind: "commandResult",
              sessionId,
              commandId: command.id,
              result,
            }));
          });
          break;
        }
        case "disconnect": {
          setStatus("disconnected", msg.reason as string);
          cleanup();
          break;
        }
        case "error": {
          setStatus("error", msg.message as string);
          break;
        }
      }
    };

    socket.onclose = () => {
      cleanup();
      scheduleReconnect();
    };

    socket.onerror = () => {
      // onclose will fire after this
    };
  }

  function cleanup(): void {
    if (heartbeatTimer) {
      clearInterval(heartbeatTimer);
      heartbeatTimer = null;
    }
    if (consoleFlushTimer) {
      clearInterval(consoleFlushTimer);
      consoleFlushTimer = null;
    }
    socket = null;
  }

  function scheduleReconnect(): void {
    if (reconnectAttempts >= maxReconnectAttempts) {
      setStatus("error", "max reconnect attempts reached");
      storage?.clear();
      return;
    }

    // Check stored session first
    const stored = storage?.get();
    if (stored && stored.expiresAtMs > Date.now() + 30_000) {
      // Token still fresh, try to reconnect with it
      reconnectAttempts++;
      const delay = Math.min(RECONNECT_BASE_MS * 2 ** (reconnectAttempts - 1), RECONNECT_MAX_MS);
      setStatus("connecting", `reconnecting in ${Math.round(delay / 1000)}s (attempt ${reconnectAttempts})`);

      reconnectTimer = setTimeout(() => {
        connect({
          sessionId: stored.sessionId,
          sessionToken: stored.sessionToken,
          socketUrl: stored.socketUrl,
          expiresAt: stored.expiresAtMs / 1000,
        });
      }, delay);
      return;
    }

    // Token expired, try auto-reconnect handshake
    if (autoReconnectHandshake) {
      reconnectAttempts++;
      const delay = Math.min(RECONNECT_BASE_MS * 2 ** (reconnectAttempts - 1), RECONNECT_MAX_MS);
      setStatus("connecting", `re-authenticating in ${Math.round(delay / 1000)}s`);

      reconnectTimer = setTimeout(async () => {
        try {
          const handshake = await autoReconnectHandshake();
          connect(handshake);
        } catch {
          setStatus("error", "re-authentication failed");
          storage?.clear();
        }
      }, delay);
      return;
    }

    setStatus("disconnected", "session expired");
    storage?.clear();
  }

  // ─── Public API ───────────────────────────────────────────────────

  function startSession(handshake: HandshakeResult): Promise<void> {
    patchConsole();
    connect(handshake);

    return new Promise<void>((resolve) => {
      // Resolve when we get the metadata (codename assigned)
      const check = setInterval(() => {
        if (status === "connected" || status === "error") {
          clearInterval(check);
          resolve();
        }
      }, 100);
    });
  }

  function disconnect(): void {
    if (reconnectTimer) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
    if (socket) {
      socket.close();
      cleanup();
    }
    unpatchConsole();
    storage?.clear();
    codename = null;
    sessionId = null;
    setStatus("disconnected");
  }

  return { startSession, disconnect, getStatus };
}
