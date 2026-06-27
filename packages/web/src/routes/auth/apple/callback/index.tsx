import type { RequestHandler } from "@builder.io/qwik-city";
import { getAppleProvider } from "~/lib/auth/providers";
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

  if (!code || !state || !storedState || state !== storedState) {
    console.log(
      "bad state",
      JSON.stringify(state),
      JSON.stringify(storedState),
    );
    throw event.redirect(302, "/auth/login?error=invalid_request");
  }

  const apple = getAppleProvider(event);
  const tokens = await apple.validateAuthorizationCode(code);

  // Apple returns user info in the ID token
  const idTokenPayload = JSON.parse(atob(tokens.idToken().split(".")[1]));

  try {
    // Create user and session via public API
    const apiClient = createApiClient(event);
    const authResult = await apiClient.nativeAuth({
      email: idTokenPayload.email,
      name: idTokenPayload.name || idTokenPayload.email?.split("@")[0],
      provider: "apple",
      provider_id: idTokenPayload.sub,
      auth_method: "apple",
      platform: "web",
      email_verified_at: idTokenPayload.email_verified
        ? new Date().toISOString()
        : undefined,
    });

    if (authResult.refresh_token) {
      const expiresAt = new Date(Date.now() + 1000 * 60 * 60 * 24 * 30); // 30 days
      setSessionTokenCookie(event, authResult.refresh_token, expiresAt);
    }

    // Clear OAuth state cookie
    event.cookie.delete("apple_oauth_state");
    event.cookie.delete("apple_oauth_redirect");
  } catch (ex) {
    console.log("apple error", JSON.stringify(ex));
    throw ex;
  }
  throw event.redirect(302, redirect);
};
