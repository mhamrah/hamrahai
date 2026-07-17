import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  startAuthentication,
  startRegistration,
} from "@simplewebauthn/browser";
import {
  WebAuthnClient,
  authenticateWithDiscoverablePasskey,
} from "./webauthn";

// Mock @simplewebauthn/browser
vi.mock("@simplewebauthn/browser", () => ({
  startRegistration: vi.fn(),
  startAuthentication: vi.fn(),
}));

// Mock @simplewebauthn/server
vi.mock("@simplewebauthn/server", () => ({
  generateRegistrationOptions: vi.fn(),
  verifyRegistrationResponse: vi.fn(),
  generateAuthenticationOptions: vi.fn(),
  verifyAuthenticationResponse: vi.fn(),
}));

// Mock public API client
vi.mock("./api-client", () => {
  const mockApiClient = {
    get: vi.fn(),
    post: vi.fn(),
    patch: vi.fn(),
    delete: vi.fn(),
  };
  return {
    createApiClient: vi.fn(() => mockApiClient),
    mockApiClient, // Export for test access
  };
});

describe("WebAuthnClient", () => {
  let webauthnClient: WebAuthnClient;
  let mockApiClient: any;
  let mockFetch: any;

  beforeEach(async () => {
    vi.clearAllMocks();

    // Mock crypto.randomUUID
    global.crypto = {
      ...global.crypto,
      randomUUID: vi.fn(
        () => "test-uuid-1234",
      ) as () => `${string}-${string}-${string}-${string}-${string}`,
    };

    // Mock fetch for WebAuthn API calls
    mockFetch = vi.fn();
    global.fetch = mockFetch;

    // Mock window.PublicKeyCredential for WebAuthn support
    Object.defineProperty(window, "PublicKeyCredential", {
      value: class MockPublicKeyCredential {},
      writable: true,
    });

    Object.defineProperty(window.navigator, "credentials", {
      value: {
        create: vi.fn(),
        get: vi.fn(),
      },
      writable: true,
    });

    // Get access to the mock API client
    const apiClientModule = (await import("./api-client")) as any;
    mockApiClient = apiClientModule.mockApiClient;

    // Create a fresh WebAuthn client instance
    webauthnClient = new WebAuthnClient();
  });

  describe("isSupported", () => {
    it("should return true when WebAuthn is supported", () => {
      expect(WebAuthnClient.isSupported()).toBe(true);
    });

    it("should return false when PublicKeyCredential is not available", () => {
      Object.defineProperty(window, "PublicKeyCredential", {
        value: undefined,
        writable: true,
      });
      expect(WebAuthnClient.isSupported()).toBe(false);
    });

    it("should return false when navigator.credentials is not available", () => {
      Object.defineProperty(window.navigator, "credentials", {
        value: undefined,
        writable: true,
      });
      expect(WebAuthnClient.isSupported()).toBe(false);
    });
  });

  describe("addPasskey", () => {
    it("starts and verifies registration with the authenticated user's details", async () => {
      mockApiClient.post
        .mockResolvedValueOnce({
          success: true,
          challenge_id: "challenge-123",
          options: {
            challenge: "ZmFrZS1jaGFsbGVuZ2U",
            rp: { id: "hamrah.app", name: "Hamrah App" },
            user: {
              id: "dXNlci0xMjM",
              name: "passkey@example.com",
              displayName: "Passkey User",
            },
            pubKeyCredParams: [{ type: "public-key", alg: -7 }],
          },
        })
        .mockResolvedValueOnce({
          success: true,
          credential_id: "credential-123",
        });
      vi.mocked(startRegistration).mockResolvedValueOnce({
        id: "credential-123",
      } as any);

      const result = await webauthnClient.addPasskey({
        id: "user-123",
        email: "passkey@example.com",
        name: "Passkey User",
      });

      expect(result).toEqual({
        success: true,
        credential_id: "credential-123",
      });
      expect(mockApiClient.post).toHaveBeenNthCalledWith(
        1,
        "/api/webauthn/register/begin",
        {
          user_id: "user-123",
          email: "passkey@example.com",
          display_name: "Passkey User",
        },
      );
      expect(mockApiClient.post).toHaveBeenNthCalledWith(
        2,
        "/api/webauthn/register/verify",
        {
          challenge_id: "challenge-123",
          response: { id: "credential-123" },
        },
      );
    });
  });

  // Removed deprecated authenticateWithPasskey test suite (email-scoped authentication no longer supported)

  describe("getUserPasskeys", () => {
    it("should return user passkeys", async () => {
      const mockCredentials = [
        { id: "cred-1", name: "Passkey 1" },
        { id: "cred-2", name: "Passkey 2" },
      ];

      mockApiClient.get.mockResolvedValue({
        success: true,
        credentials: mockCredentials,
      });

      const result = await webauthnClient.getUserPasskeys("user-123");
      expect(result).toEqual(mockCredentials);
      expect(mockApiClient.get).toHaveBeenCalledWith(
        "/api/webauthn/users/user-123/credentials",
      );
    });

    it("should return empty array on API error", async () => {
      mockApiClient.get.mockRejectedValue(new Error("API error"));

      const result = await webauthnClient.getUserPasskeys("user-123");
      expect(result).toEqual([]);
    });
  });

  describe("deletePasskey", () => {
    it("should delete passkey successfully", async () => {
      mockApiClient.delete.mockResolvedValue({ success: true });

      const result = await webauthnClient.deletePasskey("cred-123");
      expect(result).toBe(true);
      expect(mockApiClient.delete).toHaveBeenCalledWith(
        "/api/webauthn/credentials/cred-123",
      );
    });

    it("should handle delete failure", async () => {
      mockApiClient.delete.mockResolvedValue({ success: false });

      const result = await webauthnClient.deletePasskey("cred-123");
      expect(result).toBe(false);
    });

    it("should handle API error", async () => {
      mockApiClient.delete.mockRejectedValue(new Error("Network error"));

      const result = await webauthnClient.deletePasskey("cred-123");
      expect(result).toBe(false);
    });
  });

  describe("renamePasskey", () => {
    it("should rename passkey successfully", async () => {
      mockApiClient.patch.mockResolvedValue({ success: true });

      const result = await webauthnClient.renamePasskey("cred-123", "New Name");
      expect(result).toBe(true);
      expect(mockApiClient.patch).toHaveBeenCalledWith(
        "/api/webauthn/credentials/cred-123/name",
        {
          name: "New Name",
        },
      );
    });

    it("should handle rename failure", async () => {
      mockApiClient.patch.mockResolvedValue({ success: false });

      const result = await webauthnClient.renamePasskey("cred-123", "New Name");
      expect(result).toBe(false);
    });

    it("should handle API error", async () => {
      mockApiClient.patch.mockRejectedValue(new Error("Network error"));

      const result = await webauthnClient.renamePasskey("cred-123", "New Name");
      expect(result).toBe(false);
    });
  });

  describe("authenticateWithDiscoverablePasskey", () => {
    it("should return a friendly message when begin returns malformed options", async () => {
      mockApiClient.post.mockResolvedValue({
        success: true,
        options: {},
      });

      const result = await authenticateWithDiscoverablePasskey();

      expect(result).toEqual({
        success: false,
        error:
          "No passkey was found for this device. Continue with Google or Apple, then add a passkey from Settings.",
      });
      expect(startAuthentication).not.toHaveBeenCalled();
    });

    it("should not expose SimpleWebAuthn shape errors to users", async () => {
      mockApiClient.post.mockResolvedValueOnce({
        success: true,
        challenge_id: "challenge-123",
        options: {
          challenge: "ZmFrZS1jaGFsbGVuZ2U",
          rpId: "localhost",
          userVerification: "preferred",
        },
      });
      vi.mocked(startAuthentication).mockRejectedValueOnce(
        new TypeError("undefined is not an object (evaluating 'e.replace')"),
      );

      const result = await authenticateWithDiscoverablePasskey();

      expect(result).toEqual({
        success: false,
        error:
          "No passkey was found for this device. Continue with Google or Apple, then add a passkey from Settings.",
      });
    });
  });
});
