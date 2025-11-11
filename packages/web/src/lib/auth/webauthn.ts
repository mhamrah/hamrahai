// WebAuthn client implementation using @simplewebauthn/browser
// Handles passkey registration and authentication flows (registration + explicit discoverable auth)
// Now uses the API server for all WebAuthn operations

import { startAuthentication, startRegistration } from '@simplewebauthn/browser';
import { createApiClient } from './api-client';

export interface WebAuthnCredential {
  id: string;
  user_id: string;
  public_key: string;
  counter: number;
  transports?: string[];
  name?: string;
  created_at: number;
}

export interface PasskeyAuthenticationResult {
  success: boolean;
  user?: any;
  session_token?: string;
  error?: string;
}

export class WebAuthnClient {
  private apiClient = createApiClient();

  static isSupported(): boolean {
    return !!(
      window?.PublicKeyCredential &&
      window?.navigator?.credentials &&
      typeof window.navigator.credentials.create === 'function' &&
      typeof window.navigator.credentials.get === 'function'
    );
  }

  async getUserPasskeys(userId: string): Promise<WebAuthnCredential[]> {
    try {
      const response = await this.apiClient.get(`/api/webauthn/users/${userId}/credentials`);
      return response.success ? response.credentials : [];
    } catch {
      return [];
    }
  }

  async deletePasskey(credentialId: string): Promise<boolean> {
    try {
      const response = await this.apiClient.delete(`/api/webauthn/credentials/${credentialId}`);
      return response.success;
    } catch {
      return false;
    }
  }

  async renamePasskey(credentialId: string, name: string): Promise<boolean> {
    try {
      const response = await this.apiClient.patch(`/api/webauthn/credentials/${credentialId}/name`, {
        name,
      });
      return response.success;
    } catch {
      return false;
    }
  }

  async addPasskey(
    user: { id: string; email: string; name?: string },
    _opts?: { name?: string }
  ): Promise<{ success: boolean; credential_id?: string; error?: string }> {
    if (!WebAuthnClient.isSupported()) {
      return { success: false, error: 'WebAuthn is not supported in this browser' };
    }

    try {
      // Use API client instead of direct fetch
      const beginResponse: any = await this.apiClient.post('/api/webauthn/register/begin', {
        user_id: user.id,
        email: user.email,
        display_name: user.name || user.email,
      });

      if (!beginResponse.success || !beginResponse.options) {
        return { success: false, error: beginResponse.error || 'Failed to begin passkey registration' };
      }

      const registrationResponse = await startRegistration({
        optionsJSON: beginResponse.options,
      });

      if (!registrationResponse) {
        return { success: false, error: 'No credential created' };
      }

      // Use API client for verification
      const verifyResponse: any = await this.apiClient.post('/api/webauthn/register/verify', {
        challenge_id: beginResponse.challenge_id ?? beginResponse.options?.challenge_id,
        response: registrationResponse,
      });

      if (!verifyResponse.success) {
        return { success: false, error: verifyResponse.error || 'Passkey registration failed' };
      }

      return {
        success: true,
        credential_id: verifyResponse.credential_id,
      };
    } catch (e: any) {
      let msg = 'Passkey registration failed';
      if (e?.name === 'NotAllowedError') msg = 'Registration was cancelled or timed out';
      else if (e?.message) msg = e.message;
      return { success: false, error: msg };
    }
  }
}

export const webauthnClient = new WebAuthnClient();

export async function authenticateWithDiscoverablePasskey(): Promise<PasskeyAuthenticationResult> {
  if (
    !(
      (globalThis as any)?.PublicKeyCredential &&
      (globalThis as any)?.navigator?.credentials &&
      typeof (navigator as any).credentials.get === 'function'
    )
  ) {
    return {
      success: false,
      error: 'WebAuthn is not supported in this browser',
    };
  }

  try {
    const apiClient = createApiClient();

    // Use API client instead of direct fetch
    const beginResponse: any = await apiClient.post('/api/webauthn/authenticate/discoverable', {
      explicit: true,
    });

    if (!beginResponse.success || !beginResponse.options) {
      return {
        success: false,
        error: beginResponse.error || 'Failed to begin authentication',
      };
    }

    const authResponse = await startAuthentication({
      optionsJSON: beginResponse.options,
      useBrowserAutofill: false,
    });

    if (!authResponse) {
      return { success: false, error: 'No passkey selected' };
    }

    // Use API client for verification
    const completeResponse: any = await apiClient.post('/api/webauthn/authenticate/discoverable/verify', {
      challenge_id: beginResponse.options.challenge_id ?? beginResponse.challenge_id,
      response: authResponse,
      mode: 'discoverable-explicit',
    });

    if (!completeResponse.success) {
      return {
        success: false,
        error: completeResponse.error || 'Authentication failed',
      };
    }

    return {
      success: true,
      user: completeResponse.user,
      session_token: completeResponse.session_token,
    };
  } catch (error: any) {
    let errorMessage = 'Authentication failed';
    if (error?.name === 'NotAllowedError') errorMessage = 'Authentication was cancelled or not allowed';
    else if (error?.name === 'AbortError') errorMessage = 'Authentication was aborted';
    else if (error?.message) errorMessage = error.message;

    return {
      success: false,
      error: errorMessage,
    };
  }
}
