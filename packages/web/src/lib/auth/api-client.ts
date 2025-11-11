/**
 * Public API client for hamrah-api cookie-based authentication.
 * Uses external endpoints that work with both client and server-side code.
 */
import type { RequestEventCommon } from '@builder.io/qwik-city';

import type { ApiUserWire } from '@hamrah/shared';
export type ApiUser = ApiUserWire;

export interface ApiAuthResponse {
  success: boolean;
  user?: ApiUser;
  access_token?: string;
  refresh_token?: string;
  expires_in?: number;
  error?: string;
}



export interface NativeAuthRequest {
  provider: string;
  credential: string;
  email?: string;
  name?: string;
  picture?: string;
}

/**
 * Public API client for hamrah-api communication via external endpoints.
 * Uses cookie-based authentication for session validation.
 * Safe to use on both client and server side.
 */
export class HamrahApiClient {
  private baseUrl: string;
  private event?: RequestEventCommon;

  constructor(event?: RequestEventCommon, baseUrl?: string) {
    this.baseUrl = baseUrl ?? getApiBaseUrl();
    this.event = event;
  }

  private async fetchApi<T>(
    path: string,
    options: RequestInit = {},
    withCredentials = true
  ): Promise<T> {
    const url = `${this.baseUrl}${path}`;
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      ...(options.headers as Record<string, string> | undefined),
    };

    // Forward cookies for SSR (if present)
    if (withCredentials && this.event?.request.headers.has('cookie')) {
      headers['cookie'] = this.event.request.headers.get('cookie')!;
    }

    const resp = await fetch(url, {
      ...options,
      headers,
      credentials: withCredentials ? 'include' : 'same-origin',
    });

    if (!resp.ok) {
      const error = await resp.json().catch(() => ({}));
      throw new Error((error as any)?.error || (error as any)?.message || `API error: ${resp.status}`);
    }
    return resp.json();
  }

  // Public endpoint: Validate session via cookie
  async validateSession(): Promise<ApiAuthResponse> {
    return this.fetchApi<ApiAuthResponse>('/api/auth/sessions/validate', {
      method: 'GET',
    });
  }

  // Public endpoint: Logout session
  async logout(): Promise<{ success: boolean; message?: string; error?: string }> {
    try {
      const resp = await fetch(`${this.baseUrl}/api/auth/sessions/logout`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'include',
      });
      const body = (await resp.json().catch(() => ({}))) as {
        success?: boolean;
        message?: string;
        error?: string;
      };
      if (!resp.ok) {
        return { success: false, error: body?.error || 'logout_failed' };
      }
      return { success: !!body?.success, message: body?.message };
    } catch (e) {
      return { success: false, error: e instanceof Error ? e.message : 'network_error' };
    }
  }



  // Generic REST methods for public API calls
  async get<T = any>(path: string): Promise<T> {
    return this.fetchApi<T>(path, {
      method: 'GET',
    });
  }

  async post<T = any>(path: string, data?: any): Promise<T> {
    return this.fetchApi<T>(path, {
      method: 'POST',
      body: data ? JSON.stringify(data) : undefined,
    });
  }

  async patch<T = any>(path: string, data?: any): Promise<T> {
    return this.fetchApi<T>(path, {
      method: 'PATCH',
      body: data ? JSON.stringify(data) : undefined,
    });
  }

  async delete<T = any>(path: string): Promise<T> {
    return this.fetchApi<T>(path, {
      method: 'DELETE',
    });
  }

  // Public endpoint: Revoke specific token
  async revokeToken(tokenId: string): Promise<{ success: boolean; message: string }> {
    return this.fetchApi<{ success: boolean; message: string }>(
      `/api/auth/tokens/${tokenId}/revoke`,
      {
        method: 'DELETE',
      }
    );
  }

  // Public endpoint: Revoke all user tokens
  async revokeAllUserTokens(userId: string): Promise<{ success: boolean; message: string }> {
    return this.fetchApi<{ success: boolean; message: string }>(
      `/api/auth/users/${userId}/tokens/revoke`,
      {
        method: 'DELETE',
      }
    );
  }

  // Public endpoint: Native app authentication
  async nativeAuth(params: NativeAuthRequest): Promise<ApiAuthResponse> {
    return this.fetchApi<ApiAuthResponse>('/api/auth/native', {
      method: 'POST',
      body: JSON.stringify(params),
    });
  }

}

/**
 * Get the API base URL based on environment.
 * - Production: https://api.hamrah.app
 * - Development (localhost): http://localhost:8080
 */
function getApiBaseUrl(): string {
  // Explicit overrides for tests or environments
  const viteEnv = (import.meta as any)?.env;
  const override =
    (typeof window !== 'undefined' && (window as any).__API_BASE) ||
    (viteEnv?.VITE_API_BASE as string | undefined);

  if (override) return String(override);

  if (typeof window !== 'undefined') {
    const isHttps = window.location.protocol === 'https:';
    const isLocalhost =
      window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1';

    // Prefer matching scheme for localhost to avoid mixed content
    if (isLocalhost) {
      return isHttps ? 'https://localhost:8080' : 'http://localhost:8080';
    }
    if (isHttps) return 'https://api.hamrah.app';
  }

  // Default
  return 'https://api.hamrah.app';
}

/**
 * Create a public API client for cookie-based authentication.
 * Safe to use on both client and server side.
 * Automatically uses localhost:8080 when running in dev mode.
 */
export function createApiClient(event?: RequestEventCommon): HamrahApiClient {
  return new HamrahApiClient(event, getApiBaseUrl());
}
