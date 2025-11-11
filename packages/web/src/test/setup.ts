import { beforeAll, beforeEach, afterEach, afterAll, vi } from "vitest";
import type { RequestEventCommon } from "@builder.io/qwik-city";

// Global test setup for Vitest
beforeAll(() => {
  // Set up global test environment
  process.env.NODE_ENV = "test";

  // Mock environment variables
  process.env.GOOGLE_CLIENT_ID = "test-google-client-id";
  process.env.GOOGLE_CLIENT_SECRET = "test-google-client-secret";
  process.env.APPLE_CLIENT_ID = "test-apple-client-id";
  process.env.APPLE_TEAM_ID = "test-team-id";
  process.env.APPLE_KEY_ID = "test-key-id";
  process.env.APPLE_CERTIFICATE = "test-certificate";
  process.env.COOKIE_SECRET = "test-cookie-secret-32-chars-long";
});

beforeEach(() => {
  // Clear all mocks before each test
  vi.clearAllMocks();
});

afterEach(() => {
  // Clean up after each test
  vi.restoreAllMocks();
});

afterAll(() => {
  // Global cleanup
});

// Mock fetch for API tests
global.fetch = vi.fn();

// Mock crypto for token generation
Object.defineProperty(global, "crypto", {
  value: {
    randomUUID: vi.fn(() => "test-uuid-1234"),
    getRandomValues: vi.fn((array) => {
      for (let i = 0; i < array.length; i++) {
        array[i] = Math.floor(Math.random() * 256);
      }
      return array;
    }),
  },
  writable: true,
  configurable: true,
});

// Helper function to create mock request event
export function createMockRequestEvent(
  overrides: Partial<RequestEventCommon> = {},
): RequestEventCommon {
  return {
    url: new URL("https://localhost:5173/test"),
    request: new Request("https://localhost:5173/test"),
    platform: {
      env: {
        GOOGLE_CLIENT_ID: "test-google-client-id",
        GOOGLE_CLIENT_SECRET: "test-google-client-secret",
        APPLE_CLIENT_ID: "test-apple-client-id",
        APPLE_TEAM_ID: "test-team-id",
        APPLE_KEY_ID: "test-key-id",
        APPLE_CERTIFICATE: "test-certificate",
        COOKIE_SECRET: "test-cookie-secret-32-chars-long",
      },
      cf: {},
      KV: {
        get: vi.fn().mockResolvedValue(null),
        put: vi.fn().mockResolvedValue(undefined),
        delete: vi.fn().mockResolvedValue(undefined),
        list: vi.fn().mockResolvedValue({ keys: [] }),
      },
    },
    cookie: {
      get: vi.fn().mockReturnValue({ value: "test-cookie" }),
      set: vi.fn(),
      delete: vi.fn(),
      has: vi.fn().mockReturnValue(false),
    },
    json: vi.fn(),
    send: vi.fn(),
    redirect: vi.fn(),
    error: vi.fn(),
    parseBody: vi.fn().mockResolvedValue({}),
    ...overrides,
  } as any;
}

// Helper function to mock fetch responses
export function mockFetchResponse(data: any, status = 200, headers = {}) {
  return vi.fn().mockResolvedValue({
    ok: status >= 200 && status < 300,
    status,
    headers: new Headers(headers),
    json: vi.fn().mockResolvedValue(data),
    text: vi.fn().mockResolvedValue(JSON.stringify(data)),
  });
}
