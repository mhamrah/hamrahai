import type { RequestHandler } from "@builder.io/qwik-city";
import {
  buildAppleNativeAuthRequest,
  getAppleProvider,
} from "~/lib/auth/providers";
import { setSessionTokenCookie } from "~/lib/auth/session";
import { createApiClient } from "~/lib/auth/api-client";
import { safeRedirectPath } from "~/lib/auth/redirects";

// CSRF protection handled at entry point level
// Allows POST from https://appleid.apple.com to this specific route only

export const onPost: RequestHandler = async (event) => {
  // Apple sends POST request with form data
  const formData = await event.request.formData();
  const code = formData.get("code") as string;
  const state = formData.get("state") as string;
  const storedState = event.cookie.get("apple_oauth_state")?.value ?? null;
  const redirect = safeRedirectPath(
    event.cookie.get("apple_oauth_redirect")?.value,
  );
  const linkProvider =
    event.cookie.get("apple_oauth_link_provider")?.value === "true";

  if (!code || !state || !storedState || state !== storedState) {
    console.log(
      "bad state",
      JSON.stringify(state),
      JSON.stringify(storedState),
    );
    throw event.redirect(302, "/auth/login?error=invalid_request");
  }

  try {
    const apple = getAppleProvider(event);
    const tokens = await apple.validateAuthorizationCode(code);
    const idToken = tokens.idToken();
    if (!idToken) {
      throw new Error("No ID token received from Apple");
    }

    const apiClient = createApiClient(event);
    const authResult = await apiClient.nativeAuth(
      buildAppleNativeAuthRequest(idToken, linkProvider),
    );

    if (!authResult.refresh_token) {
      throw new Error("Apple sign-in did not create a session");
    }
    const expiresAt = new Date(Date.now() + 1000 * 60 * 60 * 24 * 30);
    setSessionTokenCookie(event, authResult.refresh_token, expiresAt);
  } catch (error) {
    console.error(
      "Apple sign-in could not create an API session",
      error instanceof Error ? error.message : "unknown error",
    );
    event.cookie.delete("apple_oauth_state");
    event.cookie.delete("apple_oauth_redirect");
    event.cookie.delete("apple_oauth_link_provider");
    throw event.redirect(
      302,
      "/auth/login?error=" +
        encodeURIComponent("Unable to sign in with Apple. Please try again."),
    );
  }

  event.cookie.delete("apple_oauth_state");
  event.cookie.delete("apple_oauth_redirect");
  event.cookie.delete("apple_oauth_link_provider");
  throw event.redirect(302, redirect);
};
