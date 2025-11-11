import { component$ } from "@builder.io/qwik";
import type { DocumentHead } from "@builder.io/qwik-city";
import { useUserLoader } from "../layout";
import { PasskeyManagement } from "~/components/auth/passkey-management";

export default component$(() => {
  const user = useUserLoader();

  return (
    <div class="min-h-screen bg-gray-50 py-8">
      <div class="mx-auto max-w-4xl px-4">
        <div class="rounded-lg bg-white p-6 shadow">
          <div class="mb-6 flex items-center justify-between">
            <h1 class="text-3xl font-bold text-gray-900">Account Settings</h1>
            <a
              href="/"
              class="rounded-md bg-gray-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-gray-700"
            >
              Back to Home
            </a>
          </div>

          {/* User Info Section */}
          <div class="mb-6 border-b pb-6">
            <h2 class="mb-4 text-xl font-semibold text-gray-900">
              Profile Information
            </h2>
            <div class="flex items-center space-x-4">
              {user.value.picture && (
                <img
                  src={user.value.picture}
                  alt={user.value.name}
                  width="64"
                  height="64"
                  class="h-16 w-16 rounded-full"
                />
              )}
              <div>
                <p class="text-lg font-medium text-gray-900">
                  {user.value.name}
                </p>
                <p class="text-sm text-gray-600">{user.value.email}</p>
                {user.value.provider && (
                  <p class="mt-1 text-xs text-gray-500">
                    Connected with{" "}
                    {user.value.provider === "google" ? "Google" : "Apple"}
                  </p>
                )}
              </div>
            </div>
          </div>

          {/* Security Settings */}
          <div class="space-y-6">
            <div>
              <h2 class="mb-2 text-xl font-semibold text-gray-900">
                Security Settings
              </h2>
              <p class="mb-6 text-sm text-gray-600">
                Manage your account security and authentication settings.
              </p>
            </div>

            {/* Connected Account Info */}
            <div class="rounded-lg border border-green-200 bg-green-50 p-4">
              <div class="flex items-center">
                {user.value.provider === "google" ? (
                  <>
                    <svg class="mr-2 h-5 w-5" viewBox="0 0 24 24">
                      <path
                        fill="#4285F4"
                        d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"
                      />
                      <path
                        fill="#34A853"
                        d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"
                      />
                      <path
                        fill="#FBBC05"
                        d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"
                      />
                      <path
                        fill="#EA4335"
                        d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"
                      />
                    </svg>
                    <span class="text-sm font-medium text-green-800">
                      Connected with Google
                    </span>
                  </>
                ) : (
                  <>
                    <svg
                      class="mr-2 h-5 w-5 text-green-600"
                      fill="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path d="M18.71 19.5c-.83 1.24-1.71 2.45-3.05 2.47-1.34.03-1.77-.79-3.29-.79-1.53 0-2 .77-3.27.82-1.31.05-2.3-1.32-3.14-2.53C4.25 17 2.94 12.45 4.7 9.39c.87-1.52 2.43-2.48 4.12-2.51 1.28-.02 2.5.87 3.29.87.78 0 2.26-1.07 3.81-.91.65.03 2.47.26 3.64 1.98-.09.06-2.17 1.28-2.15 3.81.03 3.02 2.65 4.03 2.68 4.04-.03.07-.42 1.44-1.38 2.83M13 3.5c.73-.83 1.94-1.46 2.94-1.5.13 1.17-.34 2.35-1.04 3.19-.69.85-1.83 1.51-2.95 1.42-.15-1.15.41-2.35 1.05-3.11z" />
                    </svg>
                    <span class="text-sm font-medium text-green-800">
                      Connected with Apple
                    </span>
                  </>
                )}
              </div>
            </div>

            {/* Passkey Management */}
            <div class="border-t pt-6">
              <PasskeyManagement
                userId={user.value.id}
                userEmail={user.value.email}
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
});

export const head: DocumentHead = {
  title: "Account Settings - Hamrah App",
  meta: [
    {
      name: "description",
      content: "Manage your account settings and security preferences",
    },
  ],
};
