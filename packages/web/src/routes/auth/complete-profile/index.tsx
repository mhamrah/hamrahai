import { component$, useSignal, $ } from "@builder.io/qwik";
import {
  routeLoader$,
  server$,
  type DocumentHead,
} from "@builder.io/qwik-city";
import { getCurrentUser } from "~/lib/auth/utils";

export const useUser = routeLoader$(async (event) => {
  const { user } = await getCurrentUser(event);
  if (!user) {
    throw event.redirect(302, "/auth/login");
  }
  return user;
});

const updateProfile = server$(async function (email: string, name: string) {
  const { getCurrentUser } = await import("~/lib/auth/utils");
  // TODO: Replace legacy DB logic with API client calls if needed

  try {
    const { user } = await getCurrentUser(this as any);
    if (!user) {
      throw new Error("Not authenticated");
    }

    // TODO: Implement user profile update via hamrah-api instead of direct database access
    // For now, return success without actual update
    console.warn(
      "Profile update not implemented - requires hamrah-api integration",
      { email, name },
    );

    return { success: true };
  } catch (error: any) {
    console.error("Profile update error:", error);
    return { success: false, error: error.message };
  }
});

export default component$(() => {
  const user = useUser();
  const isLoading = useSignal(false);
  const error = useSignal("");
  const email = useSignal(
    user.value.email.includes("@temp.com") ? "" : user.value.email || "",
  );
  const name = useSignal(
    user.value.name === "New User" ? "" : user.value.name || "",
  );

  const handleSubmit = $(async () => {
    isLoading.value = true;
    error.value = "";

    try {
      if (!email.value && !name.value) {
        throw new Error("Please provide at least an email or name");
      }

      const result = await updateProfile(email.value, name.value);

      if (!result.success) {
        throw new Error(result.error || "Failed to update profile");
      }

      // Get redirect URL from query params
      const url = new URL(window.location.href);
      const redirectUrl = url.searchParams.get("redirect") || "/";

      window.location.href = redirectUrl;
    } catch (err: any) {
      error.value = err.message;
    } finally {
      isLoading.value = false;
    }
  });

  const handleSkip = $(() => {
    const url = new URL(window.location.href);
    const redirectUrl = url.searchParams.get("redirect") || "/";
    window.location.href = redirectUrl;
  });

  return (
    <div class="flex min-h-screen items-center justify-center bg-gray-50 px-4 py-12 sm:px-6 lg:px-8">
      <div class="w-full max-w-md space-y-8">
        <div>
          <h2 class="mt-6 text-center text-3xl font-extrabold text-gray-900">
            Complete your profile
          </h2>
          <p class="mt-2 text-center text-sm text-gray-600">
            Help us personalize your experience (optional)
          </p>
        </div>

        <div class="mt-8 space-y-6">
          {error.value && (
            <div class="rounded-md border border-red-200 bg-red-50 p-3 text-sm text-red-600">
              {error.value}
            </div>
          )}

          <div class="space-y-4">
            <div>
              <label
                for="email"
                class="block text-sm font-medium text-gray-700"
              >
                Email address
              </label>
              <input
                id="email"
                type="email"
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
                Full name
              </label>
              <input
                id="name"
                type="text"
                value={name.value}
                onInput$={(e) =>
                  (name.value = (e.target as HTMLInputElement).value)
                }
                class="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 shadow-sm focus:border-indigo-500 focus:ring-indigo-500 focus:outline-none sm:text-sm"
                placeholder="Your name"
              />
            </div>
          </div>

          <div class="flex space-x-4">
            <button
              type="button"
              onClick$={handleSubmit}
              disabled={isLoading.value}
              class="flex flex-1 justify-center rounded-md border border-transparent bg-indigo-600 px-4 py-2 text-sm font-medium text-white shadow-sm hover:bg-indigo-700 focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2 focus:outline-none disabled:cursor-not-allowed disabled:opacity-50"
            >
              {isLoading.value ? (
                <>
                  <svg
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
                  Saving...
                </>
              ) : (
                "Save & Continue"
              )}
            </button>

            <button
              type="button"
              onClick$={handleSkip}
              disabled={isLoading.value}
              class="flex flex-1 justify-center rounded-md border border-gray-300 bg-white px-4 py-2 text-sm font-medium text-gray-700 shadow-sm hover:bg-gray-50 focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2 focus:outline-none disabled:cursor-not-allowed disabled:opacity-50"
            >
              Skip for now
            </button>
          </div>

          <p class="text-center text-xs text-gray-500">
            You can always update this information later in your account
            settings
          </p>
        </div>
      </div>
    </div>
  );
});

export const head: DocumentHead = {
  title: "Complete Profile - Hamrah App",
  meta: [
    {
      name: "description",
      content: "Complete your profile to personalize your experience",
    },
  ],
};
