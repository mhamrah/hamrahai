import { describe, expect, it, vi } from "vitest";
import { validateSessionToken } from "./session";

const mockValidateSession = vi.fn();

vi.mock("./api-client", () => ({
  createApiClient: vi.fn(() => ({
    validateSession: mockValidateSession,
  })),
}));

describe("validateSessionToken", () => {
  it("requires both a successful response and a user", async () => {
    mockValidateSession.mockResolvedValueOnce({
      success: true,
      user: undefined,
    });

    const result = await validateSessionToken({} as any, "session-token");

    expect(result).toEqual({
      success: false,
      is_valid: false,
      user: null,
      session: null,
      error: "Session validation failed",
    });
  });

  it("returns an authenticated session when the API returns a user", async () => {
    mockValidateSession.mockResolvedValueOnce({
      success: true,
      user: {
        id: "user-123",
        email: "user@example.com",
        created_at: "2026-01-01T00:00:00Z",
      },
    });

    const result = await validateSessionToken({} as any, "session-token");

    expect(result.success).toBe(true);
    expect(result.is_valid).toBe(true);
    expect(result.user).toEqual({
      id: "user-123",
      email: "user@example.com",
      created_at: "2026-01-01T00:00:00Z",
    });
    expect(result.session?.token).toBe("session-token");
  });
});
