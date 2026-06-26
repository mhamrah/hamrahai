/**
 * Public API client for hamrah-api cookie-based authentication.
 * Uses external endpoints that work with both client and server-side code.
 */
import type { RequestEventCommon } from "@builder.io/qwik-city";

import type { ApiUserWire } from "@hamrah/shared";
export type ApiUser = ApiUserWire;

export type ApiAuthRequirement = "none" | "optional" | "required";
export type ApiErrorCategory =
  | "unauthorized"
  | "session_expired"
  | "server"
  | "network"
  | "decoding";

export class ApiClientError extends Error {
  constructor(
    public readonly category: ApiErrorCategory,
    message: string,
    public readonly status?: number,
  ) {
    super(message);
    this.name = "ApiClientError";
  }
}

interface ApiRequestOptions extends RequestInit {
  auth?: ApiAuthRequirement;
}

function readCookie(name: string, cookieHeader?: string | null): string | null {
  if (cookieHeader) {
    const match = cookieHeader
      .split(";")
      .map((cookie) => cookie.trim())
      .find((cookie) => cookie.startsWith(`${name}=`));
    return match
      ? decodeURIComponent(match.split("=").slice(1).join("="))
      : null;
  }
  if (typeof document === "undefined") return null;
  const match = document.cookie
    .split(";")
    .map((cookie) => cookie.trim())
    .find((cookie) => cookie.startsWith(`${name}=`));
  return match ? decodeURIComponent(match.split("=").slice(1).join("=")) : null;
}

function csrfHeader(
  method?: string,
  cookieHeader?: string | null,
): Record<string, string> {
  const normalized = (method || "GET").toUpperCase();
  if (!["POST", "PUT", "PATCH", "DELETE"].includes(normalized)) return {};
  const token = readCookie("csrf_token", cookieHeader);
  return token ? { "X-CSRF-Token": token } : {};
}

export interface ApiAuthResponse {
  success: boolean;
  user?: ApiUser;
  access_token?: string;
  refresh_token?: string;
  expires_in?: number;
  expires_at?: string;
  error?: string;
}

export interface NativeAuthRequest {
  provider?: string;
  credential?: string;
  email?: string;
  name?: string;
  picture?: string;
  provider_id?: string;
  auth_method?: string;
  platform?: "web" | "ios" | "android" | "api";
  email_verified_at?: string;
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
    options: ApiRequestOptions = {},
  ): Promise<T> {
    const url = `${this.baseUrl}${path}`;
    const method = options.method || "GET";
    const auth = options.auth ?? "required";
    const cookieHeader = this.event?.request.headers.get("cookie") ?? null;
    const headers: Record<string, string> = {
      ...csrfHeader(method, cookieHeader),
      ...(options.headers as Record<string, string> | undefined),
    };
    if (options.body !== undefined && headers["Content-Type"] === undefined) {
      headers["Content-Type"] = "application/json";
    }

    // Forward cookies for SSR (if present)
    if (auth !== "none" && cookieHeader) {
      headers.cookie = cookieHeader;
    }

    let resp: Response;
    try {
      resp = await fetch(url, {
        ...options,
        headers,
        credentials: "include",
      });
    } catch (error) {
      throw new ApiClientError(
        "network",
        error instanceof Error ? error.message : "Network request failed",
      );
    }

    let body: unknown = null;
    try {
      body = await resp.json();
    } catch {
      if (resp.ok) {
        throw new ApiClientError(
          "decoding",
          "Response was not valid JSON",
          resp.status,
        );
      }
    }

    if (!resp.ok) {
      throw this.errorForResponse(resp.status, body, auth);
    }
    return body as T;
  }

  private errorForResponse(
    status: number,
    body: unknown,
    auth: ApiAuthRequirement,
  ): ApiClientError {
    const message =
      typeof body === "object" && body !== null
        ? ((body as any).error ?? (body as any).message)
        : undefined;
    if (status === 401) {
      return new ApiClientError(
        auth === "required" ? "session_expired" : "unauthorized",
        message || "Authentication required",
        status,
      );
    }
    if (status === 403) {
      return new ApiClientError(
        "unauthorized",
        message || "Unauthorized",
        status,
      );
    }
    return new ApiClientError(
      "server",
      message || `API error: ${status}`,
      status,
    );
  }

  // Public endpoint: Validate session via cookie
  async validateSession(): Promise<ApiAuthResponse> {
    return this.fetchApi<ApiAuthResponse>("/api/auth/sessions/validate", {
      method: "GET",
      auth: "required",
    });
  }

  // Public endpoint: Logout session
  async logout(): Promise<{
    success: boolean;
    message?: string;
    error?: string;
  }> {
    try {
      return await this.fetchApi<{
        success: boolean;
        message?: string;
        error?: string;
      }>("/api/auth/sessions/logout", {
        method: "POST",
        auth: "required",
      });
    } catch (e) {
      return {
        success: false,
        error: e instanceof Error ? e.message : "network_error",
      };
    }
  }

  // Generic REST methods for public API calls
  async get<T = any>(
    path: string,
    options: Pick<ApiRequestOptions, "auth"> = {},
  ): Promise<T> {
    return this.fetchApi<T>(path, {
      method: "GET",
      ...options,
    });
  }

  async post<T = any>(
    path: string,
    data?: any,
    options: Pick<ApiRequestOptions, "auth"> = {},
  ): Promise<T> {
    return this.fetchApi<T>(path, {
      method: "POST",
      body: data ? JSON.stringify(data) : undefined,
      ...options,
    });
  }

  async patch<T = any>(
    path: string,
    data?: any,
    options: Pick<ApiRequestOptions, "auth"> = {},
  ): Promise<T> {
    return this.fetchApi<T>(path, {
      method: "PATCH",
      body: data ? JSON.stringify(data) : undefined,
      ...options,
    });
  }

  async delete<T = any>(
    path: string,
    options: Pick<ApiRequestOptions, "auth"> = {},
  ): Promise<T> {
    return this.fetchApi<T>(path, {
      method: "DELETE",
      ...options,
    });
  }

  // Public endpoint: Revoke specific token
  async revokeToken(
    tokenId: string,
  ): Promise<{ success: boolean; message: string }> {
    return this.fetchApi<{ success: boolean; message: string }>(
      `/api/auth/tokens/${tokenId}/revoke`,
      {
        method: "DELETE",
      },
    );
  }

  // Public endpoint: Revoke all user tokens
  async revokeAllUserTokens(
    userId: string,
  ): Promise<{ success: boolean; message: string }> {
    return this.fetchApi<{ success: boolean; message: string }>(
      `/api/auth/users/${userId}/tokens/revoke`,
      {
        method: "DELETE",
      },
    );
  }

  // Public endpoint: Native app authentication
  async nativeAuth(params: NativeAuthRequest): Promise<ApiAuthResponse> {
    return this.fetchApi<ApiAuthResponse>("/api/auth/native", {
      method: "POST",
      body: JSON.stringify(params),
      auth: "none",
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
  const override =
    (typeof window !== "undefined" && (window as any).__API_BASE) ||
    import.meta.env.VITE_API_BASE;

  if (override) return String(override);

  if (typeof window !== "undefined") {
    const isHttps = window.location.protocol === "https:";
    const isLocalhost =
      window.location.hostname === "localhost" ||
      window.location.hostname === "127.0.0.1";

    // Prefer matching scheme for localhost to avoid mixed content
    if (isLocalhost) {
      return isHttps ? "https://localhost:8080" : "http://localhost:8080";
    }
    if (isHttps) return "https://api.hamrah.app";
  }

  // Default
  return "https://api.hamrah.app";
}

/**
 * Create a public API client for cookie-based authentication.
 * Safe to use on both client and server side.
 * Automatically uses localhost:8080 when running in dev mode.
 */
export function createApiClient(event?: RequestEventCommon): HamrahApiClient {
  return new HamrahApiClient(event, getApiBaseUrl());
}
