import { FullConfig } from "@playwright/test";

async function globalTeardown(config: FullConfig) {
  console.log("🧹 Starting E2E test teardown...");

  try {
    // Clean up test data
    console.log("🗑️ Cleaning up test data...");

    // You can add cleanup logic here such as:
    // - Removing test users
    // - Clearing test sessions
    // - Resetting database state

    console.log("✅ E2E test teardown completed");
  } catch (error) {
    console.error("❌ E2E test teardown failed:", error);
    // Don't throw here, as it would fail the entire test run
  }
}

export default globalTeardown;
