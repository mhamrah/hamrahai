import { component$, $, useSignal } from "@builder.io/qwik";
import type { DocumentHead, RequestHandler } from "@builder.io/qwik-city";
import { routeLoader$ } from "@builder.io/qwik-city";
import { UnifiedAuth } from "~/components/auth/unified-auth";

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
    redirect: redirectParam || "/",
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
    <div class="flex min-h-screen items-center justify-center bg-gray-50 px-4 py-12 sm:px-6 lg:px-8">
      <div class="w-full max-w-md space-y-8">
        {/* Header with login indication */}
        <div class="text-center">
          <button
            data-testid="login-button"
            class="cursor-default text-sm text-indigo-600 hover:text-indigo-500"
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
