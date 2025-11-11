import { component$, $ } from "@builder.io/qwik";
import { type DocumentHead } from "@builder.io/qwik-city";
import { authService } from "~/lib/auth/auth-service";
import { useUserLoader } from "./layout";

export const LogoutButton = component$(() => {
  const onLogout = $(async () => {
    try {
      await authService.logout();
    } catch {
      // Non-fatal; proceed to redirect
    } finally {
      window.location.href = "/auth/login";
    }
  });

  return (
    <button
      type="button"
      data-testid="logout-button"
      onClick$={onLogout}
      class="rounded-md bg-red-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-red-700"
    >
      Sign Out
    </button>
  );
});

export default component$(() => {
  const user = useUserLoader();

  const handleProtectedAction = $(() => {
    // Show the action result
    const resultEl = document.querySelector(
      '[data-testid="action-result"]',
    ) as HTMLElement | null;
    if (resultEl) {
      resultEl.style.display = "block";
    }
  });

  return (
    <div class="min-h-screen bg-gray-50 py-8">
      <div class="mx-auto max-w-4xl px-4">
        <div class="rounded-lg bg-white p-6 shadow">
          <div class="mb-6 flex items-center justify-between">
            <h1 class="text-3xl font-bold text-gray-900">Hamrah App</h1>
            <div class="flex space-x-3">
              <div class="relative">
                <button
                  data-testid="user-menu"
                  class="flex items-center space-x-2 rounded-md bg-gray-100 px-3 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-200"
                >
                  {user.value.picture && (
                    <img
                      data-testid="user-avatar"
                      src={user.value.picture}
                      alt={user.value.name}
                      width="24"
                      height="24"
                      class="h-6 w-6 rounded-full"
                    />
                  )}
                  <span data-testid="user-name">{user.value.name}</span>
                  <span data-testid="user-email" class="text-xs text-gray-500">
                    {user.value.email}
                  </span>
                </button>
                <div data-testid="auth-method" class="hidden">
                  {user.value.provider === "google" ? "Google" : "Apple"}
                </div>
              </div>
              <a
                href="/settings"
                data-testid="account-settings"
                class="rounded-md bg-gray-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-gray-700"
              >
                Settings
              </a>
              <LogoutButton />
            </div>
          </div>
          <div class="border-t pt-6">
            <h2 class="mb-4 text-xl font-semibold text-gray-900">
              Welcome back!
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
                <p class="mt-1 text-xs text-gray-500">
                  User ID: {user.value.id}
                </p>
                <p class="text-xs text-gray-500">
                  Signed in with{" "}
                  {user.value.provider === "google" ? "Google" : "Apple"}
                </p>
                {user.value.providerId && (
                  <p class="text-xs text-gray-500">
                    Provider ID: {user.value.providerId}
                  </p>
                )}
              </div>
            </div>
          </div>

          <div class="mt-8 border-t pt-6">
            <div class="mb-6">
              <button
                data-testid="protected-action"
                onClick$={handleProtectedAction}
                class="rounded-md bg-indigo-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-indigo-700"
              >
                Perform Protected Action
              </button>
              <div
                data-testid="action-result"
                class="mt-2 text-sm text-green-600"
                style="display: none;"
              >
                Protected action completed successfully!
              </div>
            </div>

            <h3 class="mb-4 text-lg font-semibold text-gray-900">
              User Details
            </h3>
            <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <div class="rounded-lg bg-gray-50 p-4">
                <dt class="text-sm font-medium text-gray-500">User ID</dt>
                <dd class="mt-1 font-mono text-sm break-all text-gray-900">
                  {user.value.id}
                </dd>
              </div>
              <div class="rounded-lg bg-gray-50 p-4">
                <dt class="text-sm font-medium text-gray-500">Email</dt>
                <dd class="mt-1 text-sm text-gray-900">{user.value.email}</dd>
              </div>
              <div class="rounded-lg bg-gray-50 p-4">
                <dt class="text-sm font-medium text-gray-500">Provider</dt>
                <dd class="mt-1 text-sm text-gray-900 capitalize">
                  {user.value.provider}
                </dd>
              </div>
              {user.value.providerId && (
                <div class="rounded-lg bg-gray-50 p-4">
                  <dt class="text-sm font-medium text-gray-500">Provider ID</dt>
                  <dd class="mt-1 font-mono text-sm break-all text-gray-900">
                    {user.value.providerId}
                  </dd>
                </div>
              )}
              {user.value.picture && (
                <div class="rounded-lg bg-gray-50 p-4">
                  <dt class="text-sm font-medium text-gray-500">
                    Profile Picture
                  </dt>
                  <dd class="mt-2">
                    <img
                      src={user.value.picture}
                      alt={user.value.name}
                      width="48"
                      height="48"
                      class="h-12 w-12 rounded-full"
                    />
                  </dd>
                </div>
              )}
              <div class="rounded-lg bg-gray-50 p-4">
                <dt class="text-sm font-medium text-gray-500">
                  Account Created
                </dt>
                <dd class="mt-1 text-sm text-gray-900">
                  {user.value.created_at
                    ? new Date(user.value.created_at).toLocaleDateString()
                    : "N/A"}
                </dd>
              </div>
            </div>

            <div class="mt-6">
              <p class="text-sm text-gray-600">
                You're successfully authenticated! This is your protected
                dashboard.
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
});

export const head: DocumentHead = {
  title: "Hamrah App",
  meta: [
    {
      name: "description",
      content: "Hamrah App is a playground for Qwik",
    },
  ],
};
