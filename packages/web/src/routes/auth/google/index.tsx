import type { RequestHandler } from "@builder.io/qwik-city";
import { generateState, generateCodeVerifier } from "arctic";
import { getGoogleProvider } from "~/lib/auth/providers";

export const onGet: RequestHandler = async (event) => {
  const google = getGoogleProvider(event);
  const state = generateState();
  const codeVerifier = generateCodeVerifier();

  const url = google.createAuthorizationURL(state, codeVerifier, [
    "openid",
    "profile",
    "email",
  ]);

  // Store state and code verifier in cookies for validation
  event.cookie.set("google_oauth_state", state, {
    path: "/",
    secure: event.url.protocol === "https:",
    httpOnly: true,
    maxAge: 60 * 10, // 10 minutes
    sameSite: "lax",
  });

  event.cookie.set("google_oauth_code_verifier", codeVerifier, {
    path: "/",
    secure: event.url.protocol === "https:",
    httpOnly: true,
    maxAge: 60 * 10, // 10 minutes
    sameSite: "lax",
  });

  throw event.redirect(302, url.toString());
};
