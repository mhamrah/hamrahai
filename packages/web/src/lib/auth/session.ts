import { sha256 } from "@oslojs/crypto/sha2";
import { encodeBase32LowerCaseNoPadding, encodeHexLowerCase } from "@oslojs/encoding";
import type { RequestEventCommon } from '@builder.io/qwik-city';
import { createApiClient } from "./api-client";

export function generateSessionToken(): string {
  const bytes = new Uint8Array(20);
  crypto.getRandomValues(bytes);
  const token = encodeBase32LowerCaseNoPadding(bytes);
  return token;
}

export function createSessionId(token: string): string {
  return encodeHexLowerCase(sha256(new TextEncoder().encode(token)));
}

export async function createSession(event: RequestEventCommon, token: string, userId: string) {
  // Session creation is now handled by /api/auth/native endpoint
  // This function is deprecated - use /api/auth/native for OAuth flows
  throw new Error("createSession has been moved to hamrah-api - use /api/auth/native endpoint");
}

export async function validateSessionToken(event: RequestEventCommon, token: string): Promise<SessionValidationResult> {
  try {
    // Session validation via public cookie-based endpoint
    const apiClient = createApiClient(event);
    const result = await apiClient.validateSession();

    // Convert ApiAuthResponse to SessionValidationResult
    return {
      success: result.success,
      is_valid: result.success,
      user: result.user,
      session: token ? { token, expires_at: new Date() } : null,
    };
  } catch (error) {
    console.warn('Session validation failed:', error instanceof Error ? error.message : 'Unknown error');
    // Return failed validation instead of throwing error
    return {
      success: false,
      is_valid: false,
      user: null,
      session: null,
      error: 'Session validation failed',
    };
  }
}

export function setSessionTokenCookie(event: RequestEventCommon, token: string, expires_at: Date): void {
  event.cookie.set("session", token, {
    expires: expires_at,
    sameSite: "none",  // Allow cross-site cookie sending to api.hamrah.app
    httpOnly: true,
    secure: true,
    path: "/",
    domain: ".hamrah.app",  // Share cookie with api.hamrah.app subdomain
  });
}

export function deleteSessionTokenCookie(event: RequestEventCommon): void {
  event.cookie.delete("session", { path: "/" });
}

// Cookie-based session validation (for use in routes that need Cookie access)
export async function validateSession(cookie: any): Promise<SessionValidationResult> {
  const session_token = cookie.get("session")?.value;
  if (!session_token) {
    return {
      success: false,
      is_valid: false,
      user: null,
      error: "No session token",
    };
  }

  // For now, return a basic validation. In a real implementation,
  // this would validate the session token against the API
  try {
    // This is a simplified implementation - you'd typically validate via API
    const apiClient = createApiClient();
    const result = await apiClient.validateSession();

    return {
      success: result.success,
      is_valid: result.success,
      user: result.user,
      error: result.error,
    };
  } catch (error) {
    return {
      success: false,
      is_valid: false,
      user: null,
      error: "Session validation failed",
    };
  }
}

export function invalidateSession(event: any, sessionId: string): any {
  throw new Error("invalidateSession has been moved to hamrah-api");
}

export interface SessionValidationResult {
  success: boolean;
  user?: any;
  session?: { token: string; expires_at: Date } | null;
  is_valid: boolean;
  error?: string;
}
