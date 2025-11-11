import { component$, useSignal, $ } from "@builder.io/qwik";
import type { DocumentHead, RequestHandler } from "@builder.io/qwik-city";

export const onGet: RequestHandler = async ({ cacheControl }) => {
  // Prevent caching of register page to ensure users see current auth state
  cacheControl({
    staleWhileRevalidate: 0,
    noCache: true,
    maxAge: 0,
  });
};

export default component$(() => {
  const isLoading = useSignal(false);
  const error = useSignal<string>("");
  const email = useSignal("");
  const name = useSignal("");

  // Get redirect URL from query params
  const getRedirectUrl = $(() => {
    if (typeof window !== "undefined") {
      const url = new URL(window.location.href);
      return url.searchParams.get("redirect") || "/";
    }
    return "/";
  });

  const handleRegister = $(async () => {
    if (!email.value || !name.value) {
      error.value = "Please fill in all fields";
      return;
    }

    isLoading.value = true;
    error.value = "";

    try {
      // TODO: Implement actual registration logic here
      // For now, simulate a successful registration
      await new Promise((resolve) => setTimeout(resolve, 1000));

      // Redirect to success page or dashboard
      const redirectUrl = await getRedirectUrl();
      window.location.href = redirectUrl;
    } catch (err: any) {
      let errorMessage = "Failed to register";

      if (err.message) {
        errorMessage = err.message;
      }

      error.value = errorMessage;
    } finally {
      isLoading.value = false;
    }
  });

  return (
    <div class="flex min-h-screen items-center justify-center bg-gray-50 px-4 py-12 sm:px-6 lg:px-8">
      <div class="w-full max-w-md space-y-8">
        <div class="space-y-6">
          <div class="text-center">
            <h2 class="text-3xl font-bold text-gray-900">Create Account</h2>
            <p class="mt-2 text-gray-600">Register for a new account</p>
          </div>

          {error.value && (
            <div
              data-testid="error-message"
              class="rounded-md border border-red-200 bg-red-50 p-3 text-sm text-red-600"
            >
              {error.value}
            </div>
          )}

          <div class="space-y-4">
            <div>
              <label
                for="email"
                class="block text-sm font-medium text-gray-700"
              >
                Email Address
              </label>
              <input
                id="email"
                data-testid="email-input"
                type="email"
                required
                value={email.value}
                onInput$={(e) =>
                  (email.value = (e.target as HTMLInputElement).value)
                }
                class="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 shadow-sm focus:border-indigo-500 focus:ring-indigo-500 focus:outline-none sm:text-sm"
                placeholder="your@email.com"
              />
            </div>

            <div>
              <label for="name" class="block text-sm font-medium text-gray-700">
                Full Name
              </label>
              <input
                id="name"
                data-testid="name-input"
                type="text"
                required
                value={name.value}
                onInput$={(e) =>
                  (name.value = (e.target as HTMLInputElement).value)
                }
                class="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 shadow-sm focus:border-indigo-500 focus:ring-indigo-500 focus:outline-none sm:text-sm"
                placeholder="John Doe"
              />
            </div>

            <button
              type="button"
              data-testid="register-button"
              onClick$={handleRegister}
              disabled={isLoading.value}
              class="flex w-full items-center justify-center rounded-md border border-transparent bg-indigo-600 px-4 py-3 text-sm font-medium text-white shadow-sm transition-colors hover:bg-indigo-700 focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2 focus:outline-none disabled:cursor-not-allowed disabled:opacity-50"
            >
              {isLoading.value ? (
                <>
                  <svg
                    data-testid="auth-loading"
                    class="mr-3 -ml-1 h-5 w-5 animate-spin text-white"
                    xmlns="http://www.w3.org/2000/svg"
                    fill="none"
                    viewBox="0 0 24 24"
                  >
                    <circle
                      class="opacity-25"
                      cx="12"
                      cy="12"
                      r="10"
                      stroke="currentColor"
                      stroke-width="4"
                    ></circle>
                    <path
                      class="opacity-75"
                      fill="currentColor"
                      d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                    ></path>
                  </svg>
                  Creating Account...
                </>
              ) : (
                "Register"
              )}
            </button>
          </div>

          <div class="text-center text-xs text-gray-500">
            By registering, you agree to our Terms of Service and Privacy Policy
          </div>
        </div>
      </div>
    </div>
  );
});

export const head: DocumentHead = {
  title: "Register - Hamrah App",
  meta: [
    {
      name: "description",
      content: "Create your Hamrah App account",
    },
  ],
};
