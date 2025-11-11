import type { RequestHandler } from "@builder.io/qwik-city";
import { generateState } from "arctic";
import { getAppleProvider } from "~/lib/auth/providers";

export const onGet: RequestHandler = async (event) => {
  console.log("apple");
  const apple = getAppleProvider(event);
  const state = generateState();

  const url = apple.createAuthorizationURL(state, ["name", "email"]);
  url.searchParams.set("response_mode", "form_post");

  // Store state in cookie for validation (Apple requires SameSite=None for form_post)
  event.cookie.set("apple_oauth_state", state, {
    path: "/",
    secure: true, // Required for SameSite=None
    httpOnly: true,
    maxAge: 60 * 10, // 10 minutes
    sameSite: "none",
  });

  throw event.redirect(302, url.toString());
};
