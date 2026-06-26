// WebAuthn client implementation using @simplewebauthn/browser
// Handles passkey registration and authentication flows (registration + explicit discoverable auth)
// Now uses the API server for all WebAuthn operations

import {
  startAuthentication,
  startRegistration,
} from "@simplewebauthn/browser";
import { createApiClient } from "./api-client";

const PASSKEY_SIGN_IN_UNAVAILABLE =
  "No passkey was found for this device. Continue with Google or Apple, then add a passkey from Settings.";
const PASSKEY_SETUP_UNAVAILABLE =
  "Passkey setup is not available right now. Try Google or Apple, then add a passkey from Settings.";

export interface WebAuthnCredential {
  id: string;
  user_id: string;
  public_key: string;
  counter: number;
  transports?: string[];
  name?: string;
  created_at: number | string;
  last_used?: number | string | null;
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
      typeof window.navigator.credentials.create === "function" &&
      typeof window.navigator.credentials.get === "function"
    );
  }

  async getUserPasskeys(userId: string): Promise<WebAuthnCredential[]> {
    try {
      const response = await this.apiClient.get(
        `/api/webauthn/users/${userId}/credentials`,
      );
      return response.success ? response.credentials : [];
    } catch {
      return [];
    }
  }

  async deletePasskey(credentialId: string): Promise<boolean> {
    try {
      const response = await this.apiClient.delete(
        `/api/webauthn/credentials/${credentialId}`,
      );
      return response.success;
    } catch {
      return false;
    }
  }

  async renamePasskey(credentialId: string, name: string): Promise<boolean> {
    try {
      const response = await this.apiClient.patch(
        `/api/webauthn/credentials/${credentialId}/name`,
        {
          name,
        },
      );
      return response.success;
    } catch {
      return false;
    }
  }

  async addPasskey(
    user: { id: string; email: string; name?: string },
    _opts?: { name?: string },
  ): Promise<{ success: boolean; credential_id?: string; error?: string }> {
    if (!WebAuthnClient.isSupported()) {
      return {
        success: false,
        error: "WebAuthn is not supported in this browser",
      };
    }

    try {
      // Use API client instead of direct fetch
      const beginResponse: any = await this.apiClient.post(
        "/api/webauthn/register/begin",
        {
          user_id: user.id,
          email: user.email,
          display_name: user.name || user.email,
        },
      );

      if (!beginResponse.success || !beginResponse.options) {
        return {
          success: false,
          error: beginResponse.error || "Failed to begin passkey registration",
        };
      }

      if (!hasRegistrationOptions(beginResponse.options)) {
        return { success: false, error: PASSKEY_SETUP_UNAVAILABLE };
      }

      const registrationResponse = await startRegistration({
        optionsJSON: beginResponse.options,
      });

      if (!registrationResponse) {
        return { success: false, error: "No credential created" };
      }

      // Use API client for verification
      const verifyResponse: any = await this.apiClient.post(
        "/api/webauthn/register/verify",
        {
          challenge_id:
            beginResponse.challenge_id ?? beginResponse.options?.challenge_id,
          response: registrationResponse,
        },
      );

      if (!verifyResponse.success) {
        return {
          success: false,
          error: verifyResponse.error || "Passkey registration failed",
        };
      }

      return {
        success: true,
        credential_id: verifyResponse.credential_id,
      };
    } catch (e: any) {
      return { success: false, error: passkeyRegistrationErrorMessage(e) };
    }
  }
}

export const webauthnClient = new WebAuthnClient();

export async function authenticateWithDiscoverablePasskey(): Promise<PasskeyAuthenticationResult> {
  const e2ePasskeyAuth = (globalThis as any).__HAMRAH_E2E_PASSKEY_AUTH;
  const hostname = (globalThis as any).location?.hostname;
  if (
    typeof e2ePasskeyAuth === "function" &&
    (hostname === "localhost" || hostname === "127.0.0.1")
  ) {
    return e2ePasskeyAuth();
  }

  if (
    !(
      (globalThis as any)?.PublicKeyCredential &&
      (globalThis as any)?.navigator?.credentials &&
      typeof (navigator as any).credentials.get === "function"
    )
  ) {
    return {
      success: false,
      error: "WebAuthn is not supported in this browser",
    };
  }

  try {
    const apiClient = createApiClient();

    // Use API client instead of direct fetch
    const beginResponse: any = await apiClient.post(
      "/api/webauthn/authenticate/discoverable",
      {
        explicit: true,
      },
      { auth: "none" },
    );

    if (!beginResponse.success || !beginResponse.options) {
      return {
        success: false,
        error: beginResponse.error || "Failed to begin authentication",
      };
    }

    if (!hasAuthenticationOptions(beginResponse.options)) {
      return {
        success: false,
        error: PASSKEY_SIGN_IN_UNAVAILABLE,
      };
    }

    const authResponse = await startAuthentication({
      optionsJSON: beginResponse.options,
      useBrowserAutofill: false,
    });

    if (!authResponse) {
      return { success: false, error: "No passkey selected" };
    }

    // Use API client for verification
    const completeResponse: any = await apiClient.post(
      "/api/webauthn/authenticate/discoverable/verify",
      {
        challenge_id:
          beginResponse.options.challenge_id ?? beginResponse.challenge_id,
        response: authResponse,
        mode: "discoverable-explicit",
        platform: "web",
      },
      { auth: "none" },
    );

    if (!completeResponse.success) {
      return {
        success: false,
        error: completeResponse.error || "Authentication failed",
      };
    }

    return {
      success: true,
      user: completeResponse.user,
      session_token: completeResponse.session_token,
    };
  } catch (error: any) {
    return {
      success: false,
      error: passkeyAuthenticationErrorMessage(error),
    };
  }
}

function optionsRecord(options: unknown): Record<string, any> | null {
  if (!options || typeof options !== "object") return null;
  return options as Record<string, any>;
}

function credentialOptions(options: unknown): Record<string, any> | null {
  const record = optionsRecord(options);
  if (!record) return null;
  return optionsRecord(record.publicKey) ?? record;
}

function hasChallenge(options: unknown): boolean {
  const publicKey = credentialOptions(options);
  return (
    typeof publicKey?.challenge === "string" && publicKey.challenge.length > 0
  );
}

function hasAuthenticationOptions(options: unknown): boolean {
  return hasChallenge(options);
}

function hasRegistrationOptions(options: unknown): boolean {
  const publicKey = credentialOptions(options);
  const user = optionsRecord(publicKey?.user);
  return (
    hasChallenge(options) &&
    typeof user?.id === "string" &&
    user.id.length > 0 &&
    typeof user?.name === "string" &&
    user.name.length > 0 &&
    typeof user?.displayName === "string" &&
    user.displayName.length > 0
  );
}

function isWebAuthnShapeError(error: any): boolean {
  return (
    error instanceof TypeError ||
    (typeof error?.message === "string" &&
      (error.message.includes("replace") ||
        error.message.includes("base64url")))
  );
}

function passkeyAuthenticationErrorMessage(error: any): string {
  if (error?.name === "NotAllowedError" || error?.name === "AbortError") {
    return PASSKEY_SIGN_IN_UNAVAILABLE;
  }
  if (isWebAuthnShapeError(error)) return PASSKEY_SIGN_IN_UNAVAILABLE;
  return error?.message || "Authentication failed";
}

function passkeyRegistrationErrorMessage(error: any): string {
  if (error?.name === "NotAllowedError" || error?.name === "AbortError") {
    return "Passkey setup was cancelled or timed out.";
  }
  if (isWebAuthnShapeError(error)) return PASSKEY_SETUP_UNAVAILABLE;
  return error?.message || "Passkey registration failed";
}
