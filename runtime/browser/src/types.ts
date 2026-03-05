/** Result from the handshake endpoint */
export interface HandshakeResult {
  sessionId: string;
  sessionToken: string;
  socketUrl: string;
  expiresAt: number;
}

/** Persists session info across page reloads */
export interface StorageAdapter {
  get(): StoredSession | null;
  set(session: StoredSession): void;
  clear(): void;
}

export interface StoredSession {
  sessionId: string;
  sessionToken: string;
  socketUrl: string;
  expiresAtMs: number;
  codename?: string;
}

/** Connection status */
export type ConnectionStatus = "disconnected" | "connecting" | "connected" | "error";

export interface StatusSnapshot {
  status: ConnectionStatus;
  codename?: string;
  sessionId?: string;
  reason?: string;
}

export type StatusCallback = (snapshot: StatusSnapshot) => void;

export interface BrowserTapOptions {
  /** Storage adapter for persisting session across reloads */
  storage?: StorageAdapter;
  /** Called whenever connection status changes */
  onStatus?: StatusCallback;
  /** Called to re-handshake on reconnect (returns new handshake result) */
  autoReconnectHandshake?: () => Promise<HandshakeResult>;
  /** Maximum reconnect attempts (default: 5) */
  maxReconnectAttempts?: number;
  /** Console capture enabled (default: true) */
  captureConsole?: boolean;
  /** Network capture enabled (default: false) */
  captureNetwork?: boolean;
}

export interface BrowserTapClient {
  /** Start a session with a handshake result */
  startSession(handshake: HandshakeResult): Promise<void>;
  /** Disconnect the current session */
  disconnect(): void;
  /** Get current status */
  getStatus(): StatusSnapshot;
}

// ─── Protocol messages ──────────────────────────────────────────────────────

export interface BrowserMessage {
  kind: string;
  [key: string]: unknown;
}

export interface DaemonMessage {
  kind: string;
  [key: string]: unknown;
}

export interface BrowserCommand {
  type: string;
  id: string;
  [key: string]: unknown;
}

export interface CommandResult {
  ok: boolean;
  data?: unknown;
  error?: string;
  durationMs: number;
}
