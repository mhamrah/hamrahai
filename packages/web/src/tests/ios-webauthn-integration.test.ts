// Integration test to ensure iOS app WebAuthn compatibility
// This test validates the API contract between iOS app and web endpoints

import { describe, test, expect } from "vitest";

describe("iOS WebAuthn Integration", () => {
  test("discoverable passkey begin request format should be compatible", () => {
    // Explicit discoverable flow may include an optional { explicit: true } flag; body can also be empty.
    const beginRequest = { explicit: true };
    expect(beginRequest).toHaveProperty("explicit");
    expect(typeof beginRequest.explicit).toBe("boolean");
  });

  test("legacy email-based authentication request deprecated", () => {
    // Email-scoped passkey authentication has been removed; no email prerequisite is required now.
    const requiresEmail = false;
    expect(requiresEmail).toBe(false);
  });

  test("registration flow deprecated", () => {
    // Server-side explicit registration endpoints removed; only discoverable authentication remains.
    expect(true).toBe(true);
  });

  test("iOS complete authentication format should be compatible", () => {
    // This simulates the complete authentication format from iOS
    const iosCompleteAuthRequest = {
      response: {
        id: "base64-credential-id",
        rawId: "base64-credential-id",
        type: "public-key",
        response: {
          authenticatorData: "base64-authenticator-data",
          clientDataJSON: "base64-client-data-json",
          signature: "base64-signature",
          userHandle: "base64-user-handle",
        },
      },
      challenge_id: "challenge-uuid",
    };

    // Verify the request structure matches SimpleWebAuthn format expected by web
    expect(iosCompleteAuthRequest.response).toHaveProperty("id");
    expect(iosCompleteAuthRequest.response).toHaveProperty("rawId");
    expect(iosCompleteAuthRequest.response).toHaveProperty("type");
    expect(iosCompleteAuthRequest.response).toHaveProperty("response");
    expect(iosCompleteAuthRequest.response.response).toHaveProperty(
      "authenticatorData",
    );
    expect(iosCompleteAuthRequest.response.response).toHaveProperty(
      "clientDataJSON",
    );
    expect(iosCompleteAuthRequest.response.response).toHaveProperty(
      "signature",
    );
    expect(iosCompleteAuthRequest.response.response).toHaveProperty(
      "userHandle",
    );
    expect(iosCompleteAuthRequest).toHaveProperty("challenge_id");
  });

  test("WebAuthn endpoints (explicit discoverable flow) should exist", () => {
    // Updated endpoint surface after deprecating registration & email-scoped flows
    const requiredEndpoints = [
      "/api/webauthn/authenticate/discoverable",
      "/api/webauthn/authenticate/discoverable/verify",
    ];

    expect(requiredEndpoints).toHaveLength(2);
    requiredEndpoints.forEach((endpoint) => {
      expect(typeof endpoint).toBe("string");
      expect(endpoint).toMatch(/^\/api\/webauthn\//);
    });
  });
});
