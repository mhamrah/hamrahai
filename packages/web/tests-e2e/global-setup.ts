import { chromium, FullConfig } from "@playwright/test";

async function globalSetup(config: FullConfig) {
  console.log("🧪 Starting E2E test setup...");

  // Launch a browser to perform setup tasks
  const browser = await chromium.launch();
  const page = await browser.newPage();

  try {
    // Wait for the development server to be ready
    console.log("⏳ Waiting for development server...");

    let retries = 0;
    const maxRetries = 30; // 30 seconds

    while (retries < maxRetries) {
      try {
        await page.goto("https://localhost:5173", {
          waitUntil: "networkidle",
          timeout: 10000,
        });
        console.log("✅ Development server is ready");
        break;
      } catch (error) {
        retries++;
        console.log(`⏳ Waiting for server... (${retries}/${maxRetries})`);
        await new Promise((resolve) => setTimeout(resolve, 1000));

        if (retries === maxRetries) {
          throw new Error("Development server failed to start within timeout");
        }
      }
    }

    console.log("✅ E2E test setup completed");
  } catch (error) {
    console.error("❌ E2E test setup failed:", error);
    throw error;
  } finally {
    await browser.close();
  }
}

export default globalSetup;
