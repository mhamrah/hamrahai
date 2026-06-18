import { describe, expect, it, vi, beforeEach } from "vitest";
import { HamrahApiClient } from "./api-client";

describe("HamrahApiClient", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: vi.fn().mockResolvedValue({
        success: true,
        user: {
          id: "user-123",
          email: "user@example.com",
          created_at: "2026-01-01T00:00:00Z",
        },
      }),
    });
  });

  it("forwards the incoming cookie header during SSR session validation", async () => {
    const event = {
      request: new Request("https://hamrah.app/", {
        headers: {
          cookie: "session=session-token; csrf_token=csrf-token",
        },
      }),
    } as any;
    const client = new HamrahApiClient(event, "https://api.hamrah.app");

    await client.validateSession();

    expect(fetch).toHaveBeenCalledWith(
      "https://api.hamrah.app/api/auth/sessions/validate",
      expect.objectContaining({
        credentials: "include",
        headers: expect.objectContaining({
          cookie: "session=session-token; csrf_token=csrf-token",
        }),
        method: "GET",
      }),
    );
  });
});
