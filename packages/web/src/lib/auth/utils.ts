import type { RequestEventCommon } from '@builder.io/qwik-city';
import { validateSessionToken, type SessionValidationResult } from './session';

export async function getCurrentUser(event: RequestEventCommon): Promise<SessionValidationResult> {
  const session_token = event.cookie.get("session")?.value;

  if (!session_token) {
    return { success: false, session: null, user: null, is_valid: false };
  }

  const result = await validateSessionToken(event, session_token);
  // Add is_valid property and create session object from token
  return {
    success: result.success || false,
    is_valid: result.success || false,
    session: session_token ? { token: session_token, expires_at: new Date() } : null,
    user: result.user || null,
  };
}

export function generateUserId(): string {
  const bytes = new Uint8Array(15);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, byte => byte.toString(16).padStart(2, '0')).join('');
}

export function generateRandomId(): string {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, byte => byte.toString(16).padStart(2, '0')).join('');
}
