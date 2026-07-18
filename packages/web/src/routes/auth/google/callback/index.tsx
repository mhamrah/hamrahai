import type { RequestHandler } from "@builder.io/qwik-city";
import {
  buildGoogleNativeAuthRequest,
  getGoogleProvider,
} from "~/lib/auth/providers";
import { setSessionTokenCookie } from "~/lib/auth/session";
import { createApiClient } from "~/lib/auth/api-client";
import { safeRedirectPath } from "~/lib/auth/redirects";

export const onGet: RequestHandler = async (event) => {
  const url = new URL(event.request.url);
  const code = url.searchParams.get("code");
  const state = url.searchParams.get("state");
  const error = url.searchParams.get("error");
  const errorDescription = url.searchParams.get("error_description");

  // Handle OAuth errors first
  if (error) {
    let errorMessage = "OAuth authentication failed";
    if (error === "access_denied") {
      errorMessage = "Authentication was cancelled";
    } else if (errorDescription) {
      errorMessage = decodeURIComponent(errorDescription);
    }

    throw event.redirect(
      302,
      `/auth/login?error=${encodeURIComponent(errorMessage)}`,
    );
  }

  // Production OAuth flow
  const storedState = event.cookie.get("google_oauth_state")?.value ?? null;
  const redirect = safeRedirectPath(
    event.cookie.get("google_oauth_redirect")?.value,
  );
  const codeVerifier =
    event.cookie.get("google_oauth_code_verifier")?.value ?? null;
  const linkProvider =
    event.cookie.get("google_oauth_link_provider")?.value === "true";

  if (
    !code ||
    !state ||
    !storedState ||
    !codeVerifier ||
    state !== storedState
  ) {
    console.log(
      "bad state",
      JSON.stringify(state),
      JSON.stringify(storedState),
    );
    throw event.redirect(302, "/auth/login?error=invalid_request");
  }

  const google = getGoogleProvider(event);
  const tokens = await google.validateAuthorizationCode(code, codeVerifier);

  // Extract user info from OpenID Connect ID token (more efficient than API call)
  const idToken = tokens.idToken();
  if (!idToken) {
    throw new Error("No ID token received from Google");
  }

  try {
    const apiClient = createApiClient(event);
    const authResult = await apiClient.nativeAuth(
      buildGoogleNativeAuthRequest(idToken, linkProvider),
    );

    if (!authResult.refresh_token) {
      throw new Error("Google sign-in did not create a session");
    }
    const expiresAt = new Date(Date.now() + 1000 * 60 * 60 * 24 * 30);
    setSessionTokenCookie(event, authResult.refresh_token, expiresAt);
  } catch (error) {
    console.error(
      "Google sign-in could not create an API session",
      error instanceof Error ? error.message : "unknown error",
    );
    event.cookie.delete("google_oauth_state");
    event.cookie.delete("google_oauth_code_verifier");
    event.cookie.delete("google_oauth_redirect");
    event.cookie.delete("google_oauth_link_provider");
    throw event.redirect(
      302,
      "/auth/login?error=" +
        encodeURIComponent("Unable to sign in with Google. Please try again."),
    );
  }

  // Clear OAuth cookies
  event.cookie.delete("google_oauth_state");
  event.cookie.delete("google_oauth_code_verifier");
  event.cookie.delete("google_oauth_redirect");
  event.cookie.delete("google_oauth_link_provider");

  throw event.redirect(302, redirect);
};
