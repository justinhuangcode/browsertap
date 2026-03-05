import type { StorageAdapter, StoredSession } from "./types";

const STORAGE_KEY = "browsertap:session";

/** SessionStorage-based adapter (survives page reloads within the same tab) */
export function createSessionStorageAdapter(): StorageAdapter {
  return {
    get(): StoredSession | null {
      try {
        const raw = sessionStorage.getItem(STORAGE_KEY);
        return raw ? JSON.parse(raw) : null;
      } catch {
        return null;
      }
    },
    set(session: StoredSession): void {
      try {
        sessionStorage.setItem(STORAGE_KEY, JSON.stringify(session));
      } catch {
        // Storage full or unavailable
      }
    },
    clear(): void {
      try {
        sessionStorage.removeItem(STORAGE_KEY);
      } catch {
        // Ignore
      }
    },
  };
}

/** In-memory adapter (does not survive page reloads) */
export function createMemoryStorageAdapter(): StorageAdapter {
  let stored: StoredSession | null = null;
  return {
    get: () => stored,
    set: (session) => { stored = session; },
    clear: () => { stored = null; },
  };
}
