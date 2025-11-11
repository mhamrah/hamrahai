/*
 * WHAT IS THIS FILE?
 *
 * It's the entry point for Cloudflare Pages when building for production.
 *
 * Learn more about the Cloudflare Pages integration here:
 * - https://qwik.dev/docs/deployments/cloudflare-pages/
 *
 */
import {
  createQwikCity,
  type PlatformCloudflarePages,
} from "@builder.io/qwik-city/middleware/cloudflare-pages";
import qwikCityPlan from "@qwik-city-plan";
import { manifest } from "@qwik-client-manifest";
import render from "./entry.ssr";
// Local fallback Env type for Cloudflare Worker bindings.
// If you generate Worker types (e.g., via `wrangler types`) or define worker-configuration.d.ts,
// remove this local alias and rely on the generated Env instead.
type Env = Record<string, unknown>;
declare global {
  interface QwikCityPlatform extends PlatformCloudflarePages {
    env: Env;
  }
}

const qwikCityFetch = createQwikCity({ render, qwikCityPlan, manifest });

// Custom fetch handler to allow Apple OAuth while maintaining CSRF protection
const fetch = async (request: Request, env: Env, ctx: ExecutionContext) => {
  const url = new URL(request.url);
  const origin = request.headers.get("origin");

  // Allow Apple OAuth POST to callback endpoint specifically
  if (
    request.method === "POST" &&
    url.pathname === "/auth/apple/callback" &&
    origin === "https://appleid.apple.com"
  ) {
    // Create modified headers to bypass CSRF for Apple OAuth only
    const modifiedHeaders = new Headers(request.headers);
    modifiedHeaders.set("origin", url.origin); // Match server origin

    const modifiedRequest = new Request(request.url, {
      method: request.method,
      headers: modifiedHeaders,
      body: request.body,
    });

    return qwikCityFetch(modifiedRequest, env as any, ctx);
  }

  // Default handling for all other requests (maintains CSRF protection)
  return qwikCityFetch(request, env as any, ctx);
};

export { fetch };
