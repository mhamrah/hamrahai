import type { RequestHandler } from "@builder.io/qwik-city";
import { getGoogleProvider } from "~/lib/auth/providers";
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

  const idTokenPayload = JSON.parse(atob(idToken.split(".")[1]));

  // OpenID Connect standard claims + Google-specific claims
  const googleUser = {
    sub: idTokenPayload.sub, // Subject (unique user ID)
    email: idTokenPayload.email,
    email_verified: idTokenPayload.email_verified,
    name: idTokenPayload.name,
    given_name: idTokenPayload.given_name, // First name
    family_name: idTokenPayload.family_name, // Last name
    picture: idTokenPayload.picture,
    locale: idTokenPayload.locale, // Language preference
    hd: idTokenPayload.hd, // Hosted domain (for Google Workspace users)
  };

  // Additional claims available but not currently stored:
  // - aud: Audience (your client_id)
  // - iss: Issuer (https://accounts.google.com)
  // - iat: Issued at time
  // - exp: Expiration time
  // - at_hash: Access token hash

  try {
    // Create user and session via public API
    const apiClient = createApiClient(event);
    const authResult = await apiClient.post("/api/auth/native", {
      email: googleUser.email,
      name: googleUser.name,
      picture: googleUser.picture,
      provider: "google",
      provider_id: googleUser.sub,
      auth_method: "google",
      platform: "web",
      email_verified_at: googleUser.email_verified
        ? new Date().toISOString()
        : undefined,
    });

    if (authResult.refresh_token) {
      const expiresAt = new Date(Date.now() + 1000 * 60 * 60 * 24 * 30); // 30 days
      setSessionTokenCookie(event, authResult.refresh_token, expiresAt);
    }
  } catch (error) {
    console.log("could not authenticate with API", error);
    throw error;
  }

  // Clear OAuth cookies
  event.cookie.delete("google_oauth_state");
  event.cookie.delete("google_oauth_code_verifier");
  event.cookie.delete("google_oauth_redirect");

  throw event.redirect(302, redirect);
};
