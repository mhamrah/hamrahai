/// <reference types="vitest" />
import { defineConfig } from "vitest/config";
import { qwikVite } from "@builder.io/qwik/optimizer";
import { qwikCity } from "@builder.io/qwik-city/vite";
import tsconfigPaths from "vite-tsconfig-paths";

export default defineConfig({
  plugins: [
    qwikCity({
      platform: {
        // Mock platform for testing
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
        delete: async () => {},
        list: async () => ({ keys: [] }),
      },
    }),
    qwikVite(),
    tsconfigPaths(),
  ],
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    include: ["src/**/*.{test,spec}.{js,ts,tsx}"],
    exclude: ["node_modules", "dist", ".qwik", "tests-e2e"],
    coverage: {
      provider: "v8",
      reporter: ["text", "json", "html"],
      include: ["src/**/*"],
      exclude: [
        "src/**/*.{test,spec}.{js,ts,tsx}",
        "src/test/**/*",
        "src/entry.*.tsx",
        "src/global.css",
      ],
    },
    // Test timeout for integration tests
    testTimeout: 30000,
    // Hooks timeout for setup/teardown
    hookTimeout: 30000,
  },
  esbuild: {
    target: "esnext",
  },
});
