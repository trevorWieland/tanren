import type { WindowContextId } from "@/app/lib/generated/account-contract";
import { parseWindowContextId as parseGeneratedWindowContextId } from "@/app/lib/generated/account-contract";

const WINDOW_ID_HEADER = "x-tanren-window-id";
const WINDOW_ID_STORAGE_KEY = "tanren.window_id";

/**
 * Strategy interface for managing browser window context identity.
 * Implementations control how window IDs are resolved, cached,
 * rotated, and cleared across the authentication lifecycle.
 */
export interface WindowContextStrategy {
  /** Resolve the current window context ID. Returns null when unavailable. */
  resolve(): WindowContextId | null;
  /** Rotate the window context ID after authentication success. */
  rotate(): void;
  /** Clear the window context ID after sign-out. */
  clear(): void;
}

/**
 * Build the window-identity request header from a resolved strategy.
 * Throws when no window context is available.
 */
export function identityHeader(
  strategy: WindowContextStrategy,
): Record<string, string> {
  const windowId = strategy.resolve();
  if (windowId === null) {
    throw new Error("Unable to initialize browser window context.");
  }
  return { [WINDOW_ID_HEADER]: windowId };
}

/** HTTP header name used for window-context identity. */
export const windowIdHeaderName = WINDOW_ID_HEADER;

// -- Browser sessionStorage implementation --

let cachedWindowId: WindowContextId | null = null;

function resolveBrowserWindowId(): WindowContextId | null {
  if (typeof window === "undefined") {
    return null;
  }
  try {
    const existingRaw = window.sessionStorage.getItem(WINDOW_ID_STORAGE_KEY);
    if (existingRaw !== null) {
      const existing = parseWindowContextId(existingRaw);
      if (existing !== null) {
        return existing;
      }
      clearBrowserWindowId();
    }

    if (typeof globalThis.crypto?.randomUUID !== "function") {
      return null;
    }
    const created = parseWindowContextId(globalThis.crypto.randomUUID());
    if (created === null) {
      return null;
    }
    window.sessionStorage.setItem(WINDOW_ID_STORAGE_KEY, created);
    return created;
  } catch {
    return null;
  }
}

function clearBrowserWindowId(): void {
  cachedWindowId = null;
  if (typeof window === "undefined") {
    return;
  }
  try {
    window.sessionStorage.removeItem(WINDOW_ID_STORAGE_KEY);
  } catch {
    // Ignore storage access errors; callers already surface failures.
  }
}

function rotateBrowserWindowId(): void {
  clearBrowserWindowId();
  void resolveBrowserWindowId();
}

function getCachedBrowserWindowId(): WindowContextId | null {
  if (cachedWindowId !== null) {
    return cachedWindowId;
  }
  const resolved = resolveBrowserWindowId();
  if (resolved !== null) {
    cachedWindowId = resolved;
  }
  return resolved;
}

/**
 * Default browser-based window context strategy using sessionStorage.
 * Window IDs are stable within a page session and rotate on explicit
 * sign-out or sign-in transitions.
 */
export const browserWindowContext: WindowContextStrategy = {
  resolve(): WindowContextId | null {
    return getCachedBrowserWindowId();
  },
  rotate(): void {
    rotateBrowserWindowId();
  },
  clear(): void {
    clearBrowserWindowId();
  },
};

/**
 * Resolve the cached window context ID for the current browser session.
 * Returns null when window context is unavailable (SSR, missing crypto,
 * storage errors).
 */
export function getCachedWindowId(): WindowContextId | null {
  return browserWindowContext.resolve();
}

/**
 * Invalidate the cached window context, forcing re-resolution on the
 * next call.
 */
export function invalidateCachedWindowId(): void {
  cachedWindowId = null;
}

/**
 * Parse a value into a WindowContextId using the generated contract
 * validator. Returns null when the value is not a valid UUID.
 */
export function parseWindowContextId(payload: unknown): WindowContextId | null {
  return parseGeneratedWindowContextId(payload);
}
