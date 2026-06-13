/**
 * AuthService - Thin client-side/session-aware wrapper around hamrah-api
 *
 * Centralizes:
 *  - Session validation
 *  - Current user retrieval / caching
 *  - Logout
 *  - Login / logout event subscription
 *
 * Wire protocol rules (enforced here):
 *  - All outbound / inbound JSON keys to the API are snake_case.
 *  - All timestamps are RFC 3339 UTC strings (parsed where needed).
 *  - CSRF double-submit supported: expects a non-HttpOnly cookie named 'csrf_token'
 *    set by hamrah-api and echoes it via 'X-CSRF-Token' on unsafe methods
 *    (POST, PUT, PATCH, DELETE) with credentials: 'include'.
 *
 * This service intentionally does NOT expose:
 *  - Token creation / refresh (handled by hamrah-api; web relies on HttpOnly session cookie)
 *  - Local secure storage (web has no secure enclave; rely on cookie only)
 *
 * Dependencies:
 *  - Shared DTOs from @hamrah/shared for strong typing
 *  - Global fetch (browser / Cloudflare environment)
 *
 * Usage (client components or loaders):
 *    import { authService } from '~/lib/auth/auth-service';
 *
 *    const session = await authService.validateSession();
 *    if (session.authenticated) {
 *       console.log(session.user?.email);
 *    }
 *
 *    await authService.logout();
 *
 * Events:
 *    const unsub = authService.subscribe((evt) => {
 *       if (evt.type === 'logout') { ... }
 *    });
 *    unsub();
 */

import type {
  ApiUserWire,
  SessionValidationResponse,
} from '@hamrah/shared';

// CSRF double-submit configuration and helpers
// Backend should set a non-HttpOnly CSRF cookie with this name.
const CSRF_COOKIE_NAME = 'csrf_token';
// Client echoes the cookie value in this header on unsafe methods.
const CSRF_HEADER_NAME = 'X-CSRF-Token';

/**
 * Read a cookie value by name (non-HttpOnly cookies only).
 */
function readCookie(name: string): string | null {
  if (typeof document === 'undefined') return null;
  const match = document.cookie
    .split(';')
    .map((c) => c.trim())
    .find((c) => c.startsWith(name + '='));
  if (!match) return null;
  return decodeURIComponent(match.split('=').slice(1).join('='));
}

/**
 * Get CSRF token from non-HttpOnly cookie (set by hamrah-api).
 */
function getCsrfTokenFromCookie(): string | null {
  return readCookie(CSRF_COOKIE_NAME);
}

/* -------------------------------------------------------------------------- */
/*                                Type Aliases                                */
/* -------------------------------------------------------------------------- */

export interface AuthSessionState {
  authenticated: boolean;
  user: ApiUserWire | null;
  // When the session was last validated (ms epoch)
  validatedAt: number | null;
  // Raw expires_at from API if provided
  expiresAt?: string;
  // Error message if last validation failed
  error?: string;
}

export interface AuthEventLogin {
  type: 'login';
  user: ApiUserWire;
}

export interface AuthEventLogout {
  type: 'logout';
  reason: 'explicit' | 'session_invalid' | 'network_error';
}

export interface AuthEventRefresh {
  type: 'refresh';
  user: ApiUserWire | null;
}

export type AuthEvent = AuthEventLogin | AuthEventLogout | AuthEventRefresh;

export interface ValidateOptions {
  /**
   * Force bypass of cached user snapshot even if recently validated.
   */
  force?: boolean;
  /**
   * Reuse cached result if validated within this many milliseconds (default 10s).
   */
  maxAgeMs?: number;
}

/* -------------------------------------------------------------------------- */
/*                            Internal Helper Logic                           */
/* -------------------------------------------------------------------------- */

/**
 * Resolve hamrah-api base URL (mirrors logic in the existing api-client).
 * Prefers:
 *   1. window.__API_BASE (test overrides)
 *   2. Vite env VITE_API_BASE
 *   3. https://api.hamrah.app (production)
 *   4. http://localhost:8080 (dev heuristic)
 */
function resolveApiBase(): string {
  // Runtime override (tests / diagnostics)
  if (typeof window !== 'undefined') {
    const win = window as unknown as { __API_BASE?: string };
    if (win.__API_BASE) return win.__API_BASE;
  }

  if (import.meta.env.VITE_API_BASE) return import.meta.env.VITE_API_BASE;

  if (typeof window !== 'undefined') {
    const isLocal =
      window.location.hostname === 'localhost' ||
      window.location.hostname === '127.0.0.1';
    if (isLocal) {
      return window.location.protocol === 'https:'
        ? 'https://localhost:8080'
        : 'http://localhost:8080';
    }
  }

  return 'https://api.hamrah.app';
}

/**
 * Perform a JSON fetch with credentials included.
 */
async function fetchJson<T>(
  path: string,
  init: RequestInit & { expectedStatuses?: number[] } = {},
): Promise<{ ok: boolean; status: number; json: T | null }> {
  const base = resolveApiBase();
  const url = `${base}${path}`;
  const fetchInit: RequestInit & { expectedStatuses?: number[] } = { ...init };
  delete fetchInit.expectedStatuses;
  delete fetchInit.headers;
  // Build headers and attach CSRF token for unsafe methods if present
  const method = (init.method || 'GET').toString().toUpperCase();
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...(init.headers as Record<string, string> | undefined),
  };
  if (method === 'POST' || method === 'PUT' || method === 'PATCH' || method === 'DELETE') {
    const csrf = getCsrfTokenFromCookie();
    if (csrf) {
      headers[CSRF_HEADER_NAME] = csrf;
    }
  }

  const resp = await fetch(url, {
    ...fetchInit,
    credentials: 'include',
    headers,
  });

  let parsed: T | null = null;
  try {
    parsed = (await resp.json()) as T;
  } catch {
    // ignore JSON parse failures; treat as null
  }
  return { ok: resp.ok, status: resp.status, json: parsed };
}

/* -------------------------------------------------------------------------- */
/*                                Auth Service                                */
/* -------------------------------------------------------------------------- */

class AuthService {
  private state: AuthSessionState = {
    authenticated: false,
    user: null,
    validatedAt: null,
  };

  private subscribers: Set<(e: AuthEvent) => void> = new Set();

  /**
   * Subscribe to auth events (login, logout, refresh).
   * Returns an unsubscribe function.
   */
  subscribe(handler: (event: AuthEvent) => void): () => void {
    this.subscribers.add(handler);
    return () => {
      this.subscribers.delete(handler);
    };
  }

  private emit(event: AuthEvent) {
    for (const sub of this.subscribers) {
      try {
        sub(event);
      } catch (e) {
        // eslint-disable-next-line no-console
        console.error('AuthService subscriber error', e);
      }
    }
  }

  /**
   * Get the current cached snapshot (does not trigger network).
   */
  snapshot(): AuthSessionState {
    return { ...this.state };
  }

  /**
   * Validate the current session via hamrah-api.
   * Applies caching unless force is true or stale.
   */
  async validateSession(
    opts: ValidateOptions = {},
  ): Promise<AuthSessionState> {
    const { force = false, maxAgeMs = 10_000 } = opts;
    const now = Date.now();

    if (
      !force &&
      this.state.validatedAt !== null &&
      now - this.state.validatedAt < maxAgeMs
    ) {
      return this.snapshot();
    }

    const { ok, json } = await fetchJson<SessionValidationResponse>(
      '/api/auth/sessions/validate',
      { method: 'GET' },
    );

    if (!ok || !json) {
      // Consider this a soft failure; preserve previous state but mark error
      const prevAuthenticated = this.state.authenticated;
      this.state = {
        ...this.state,
        authenticated: false,
        user: null,
        validatedAt: now,
        error: 'session_validation_failed',
      };
      if (prevAuthenticated) {
        this.emit({ type: 'logout', reason: 'session_invalid' });
      } else {
        this.emit({ type: 'refresh', user: null });
      }
      return this.snapshot();
    }

    if (json.success && json.user) {
      const wasAuthenticated = this.state.authenticated;
      this.state = {
        authenticated: true,
        user: json.user,
        validatedAt: now,
        expiresAt: json.expires_at,
      };
      if (!wasAuthenticated) {
        this.emit({ type: 'login', user: json.user });
      } else {
        this.emit({ type: 'refresh', user: json.user });
      }
    } else {
      const wasAuthenticated = this.state.authenticated;
      this.state = {
        authenticated: false,
        user: null,
        validatedAt: now,
        error: json.error ?? 'unauthenticated',
      };
      if (wasAuthenticated) {
        this.emit({ type: 'logout', reason: 'session_invalid' });
      } else {
        this.emit({ type: 'refresh', user: null });
      }
    }

    return this.snapshot();
  }

  /**
   * Return cached user, optionally validating first.
   */
  async getUser(options?: ValidateOptions): Promise<ApiUserWire | null> {
    await this.validateSession(options ?? {});
    return this.state.user;
  }

  /**
   * Explicit logout via hamrah-api. Always attempts to clear server session.
   * Emits logout event even if server responds with an error (optimistic).
   */
  async logout(): Promise<{ success: boolean; error?: string }> {
    const { ok, json } = await fetchJson<{ success?: boolean; error?: string }>(
      '/api/auth/sessions/logout',
      { method: 'POST' },
    );

    const success = !!json?.success && ok;
    // Invalidate local state regardless
    const wasAuthenticated = this.state.authenticated;
    this.state = {
      authenticated: false,
      user: null,
      validatedAt: Date.now(),
    };

    if (wasAuthenticated) {
      this.emit({ type: 'logout', reason: success ? 'explicit' : 'network_error' });
    } else {
      // If already logged out, still emit refresh to notify listeners of state change
      this.emit({ type: 'refresh', user: null });
    }

    return { success, error: success ? undefined : json?.error || 'logout_failed' };
  }

  /**
   * Convenience: ensure session is valid (force validate) and return boolean.
   */
  async isAuthenticated(force = false): Promise<boolean> {
    const snapshot = await this.validateSession({ force });
    return snapshot.authenticated;
  }

  /**
   * Internal utility for debugging / metrics (optional).
   */
  debugState(): AuthSessionState {
    return this.snapshot();
  }
}

/* -------------------------------------------------------------------------- */
/*                              Singleton Export                              */
/* -------------------------------------------------------------------------- */

export const authService = new AuthService();

/* -------------------------------------------------------------------------- */
/*                               Example Patterns                             */
/* -------------------------------------------------------------------------- */
/*
  Example: Guard in a Qwik route (server-side loader) might use cookie-based
  validation directly (existing layout.tsx approach). On the client, after
  hydration, you can "warm" the session:

    await authService.validateSession();

  Example: Listening for logout in a component:

    useVisibleTask$(() => {
      const unsub = authService.subscribe(evt => {
        if (evt.type === 'logout') {
          window.location.href = '/auth/login';
        }
      });
      return () => unsub();
    });

  Example: Manual refresh button:

    const onRefresh = $(async () => {
       await authService.validateSession({ force: true });
    });
*/
