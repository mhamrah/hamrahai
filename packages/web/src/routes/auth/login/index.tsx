import { component$, $, useSignal } from "@builder.io/qwik";
import type { DocumentHead, RequestHandler } from "@builder.io/qwik-city";
import { routeLoader$ } from "@builder.io/qwik-city";
import { UnifiedAuth } from "~/components/auth/unified-auth";
import { getSafeRedirectPath } from "~/lib/auth/redirect";

export const onGet: RequestHandler = async ({ cacheControl }) => {
  // Prevent caching of login page to ensure users see current auth state
  cacheControl({
    staleWhileRevalidate: 0,
    noCache: true,
    maxAge: 0,
  });
};

export const useErrorLoader = routeLoader$(async ({ url }) => {
  const errorParam = url.searchParams.get("error");
  const errorDescriptionParam = url.searchParams.get("error_description");
  const redirectParam = url.searchParams.get("redirect");

  let errorMessage = null;

  if (errorParam) {
    // Handle OAuth-style errors
    if (errorParam === "access_denied") {
      errorMessage = "Authentication was cancelled";
    } else if (errorDescriptionParam) {
      errorMessage = decodeURIComponent(errorDescriptionParam);
    } else {
      errorMessage = decodeURIComponent(errorParam);
    }
  }

  return {
    error: errorMessage,
    redirect: getSafeRedirectPath(redirectParam),
  };
});

export default component$(() => {
  const loaderData = useErrorLoader();
  const initialError = useSignal<string>(loaderData.value.error || "");

  const handleAuthSuccess = $((user: any) => {
    console.log("Authentication successful:", user);
  });

  const handleAuthError = $((error: string) => {
    console.error("Authentication failed:", error);
    initialError.value = error;
  });

  return (
    <div class="grid min-h-screen bg-slate-50 px-4 py-8 lg:grid-cols-[1fr_440px] lg:px-0 lg:py-0">
      <section class="hidden items-center px-12 lg:flex">
        <div class="max-w-xl">
          <div class="mb-8 inline-flex rounded-full border border-cambridge-blue-200 bg-white/80 px-3 py-1 text-sm font-medium text-cambridge-blue-800 shadow-sm">
            Private, fast, passwordless
          </div>
          <h1 class="text-5xl font-semibold tracking-tight text-gray-950">
            Your knowledge workspace, ready when you are.
          </h1>
          <p class="mt-5 text-lg leading-8 text-gray-600">
            Hamrah keeps account access simple: passkeys first, trusted OAuth
            fallback, and secure sessions handled by the API.
          </p>
        </div>
      </section>

      <main class="flex items-center justify-center lg:bg-white lg:px-10">
        <div class="w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 shadow-lg lg:border-0 lg:p-0 lg:shadow-none">
          <div class="mb-8 flex items-center justify-between">
            <a
              href="/"
              class="text-lg font-semibold tracking-tight text-gray-950"
            >
              Hamrah
            </a>
            <button
              data-testid="login-button"
              class="cursor-default rounded-full bg-gray-100 px-3 py-1 text-xs font-medium text-gray-600"
              disabled
            >
              Sign In Required
            </button>
          </div>
          <UnifiedAuth
            onSuccess={handleAuthSuccess}
            onError={handleAuthError}
            redirectUrl={loaderData.value.redirect}
            initialError={initialError.value}
          />
        </div>
      </main>
    </div>
  );
});

export const head: DocumentHead = {
  title: "Sign In - Hamrah App",
  meta: [
    {
      name: "description",
      content: "Sign in to your Hamrah App account",
    },
  ],
};
