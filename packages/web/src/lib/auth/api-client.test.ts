import { describe, expect, it, vi, beforeEach } from "vitest";
import { ApiClientError, HamrahApiClient } from "./api-client";

describe("HamrahApiClient", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.unstubAllGlobals();
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

  it("does not attach CSRF to safe GET requests", async () => {
    vi.stubGlobal("document", { cookie: "csrf_token=csrf-token" });
    const client = new HamrahApiClient(undefined, "https://api.hamrah.app");

    await client.get("/api/auth/sessions/validate");

    expect(fetch).toHaveBeenCalledWith(
      "https://api.hamrah.app/api/auth/sessions/validate",
      expect.objectContaining({
        credentials: "include",
        headers: expect.not.objectContaining({
          "Content-Type": "application/json",
          "X-CSRF-Token": "csrf-token",
        }),
        method: "GET",
      }),
    );
  });

  it("attaches CSRF to unsafe browser requests", async () => {
    vi.stubGlobal("document", { cookie: "csrf_token=csrf-token" });
    const client = new HamrahApiClient(undefined, "https://api.hamrah.app");

    await client.post("/api/auth/sessions/logout");

    expect(fetch).toHaveBeenCalledWith(
      "https://api.hamrah.app/api/auth/sessions/logout",
      expect.objectContaining({
        credentials: "include",
        headers: expect.objectContaining({
          "X-CSRF-Token": "csrf-token",
        }),
        method: "POST",
      }),
    );
  });

  it("attaches JSON content type when a request has a body", async () => {
    vi.stubGlobal("document", { cookie: "csrf_token=csrf-token" });
    const client = new HamrahApiClient(undefined, "https://api.hamrah.app");

    await client.post("/api/auth/native", { provider: "google" });

    expect(fetch).toHaveBeenCalledWith(
      "https://api.hamrah.app/api/auth/native",
      expect.objectContaining({
        body: JSON.stringify({ provider: "google" }),
        headers: expect.objectContaining({
          "Content-Type": "application/json",
          "X-CSRF-Token": "csrf-token",
        }),
        method: "POST",
      }),
    );
  });

  it("forwards SSR cookies for optional native auth account linking", async () => {
    const event = {
      request: new Request("https://hamrah.app/", {
        headers: {
          cookie: "session=session-token; csrf_token=ssr-csrf-token",
        },
      }),
    } as any;
    const client = new HamrahApiClient(event, "https://api.hamrah.app");

    await client.nativeAuth({
      provider: "google",
      platform: "web",
      id_token: "id-token",
    } as any);

    expect(fetch).toHaveBeenCalledWith(
      "https://api.hamrah.app/api/auth/native",
      expect.objectContaining({
        credentials: "include",
        headers: expect.objectContaining({
          cookie: "session=session-token; csrf_token=ssr-csrf-token",
          "X-CSRF-Token": "ssr-csrf-token",
        }),
        method: "POST",
      }),
    );
  });

  it("uses SSR cookies as the CSRF source for unsafe server requests", async () => {
    const event = {
      request: new Request("https://hamrah.app/", {
        headers: {
          cookie: "session=session-token; csrf_token=ssr-csrf-token",
        },
      }),
    } as any;
    const client = new HamrahApiClient(event, "https://api.hamrah.app");

    await client.post("/api/auth/sessions/logout");

    expect(fetch).toHaveBeenCalledWith(
      "https://api.hamrah.app/api/auth/sessions/logout",
      expect.objectContaining({
        credentials: "include",
        headers: expect.objectContaining({
          cookie: "session=session-token; csrf_token=ssr-csrf-token",
          "X-CSRF-Token": "ssr-csrf-token",
        }),
        method: "POST",
      }),
    );
  });

  it("maps required-auth 401s to session_expired errors", async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: false,
      status: 401,
      json: vi.fn().mockResolvedValue({ error: "Authentication required" }),
    });
    const client = new HamrahApiClient(undefined, "https://api.hamrah.app");

    await expect(client.validateSession()).rejects.toMatchObject({
      category: "session_expired",
      status: 401,
    } satisfies Partial<ApiClientError>);
  });

  it("accepts an empty 204 response when disconnecting a provider", async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      status: 204,
      json: vi.fn(),
    });
    const client = new HamrahApiClient(undefined, "https://api.hamrah.app");

    await expect(
      client.delete("/v1/music/connections/spotify"),
    ).resolves.toBeUndefined();
  });
});
