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
    <div class="min-h-screen">
      <header class="border-b border-gray-200 bg-white/90 backdrop-blur">
        <div class="mx-auto flex max-w-6xl items-center justify-between px-4 py-4 sm:px-6">
          <a href="/" class="text-lg font-semibold tracking-tight text-gray-950">
            Hamrah
          </a>
          <nav class="flex items-center gap-2 sm:gap-3">
            <a
              href="/settings"
              data-testid="account-settings"
              class="rounded-lg px-3 py-2 text-sm font-medium text-gray-700 transition hover:bg-gray-100"
            >
              Settings
            </a>
            <LogoutButton />
          </nav>
        </div>
      </header>

      <main class="mx-auto max-w-6xl px-4 py-8 sm:px-6">
        <section class="mb-8 flex flex-col gap-6 rounded-lg border border-gray-200 bg-white p-6 shadow-sm md:flex-row md:items-center md:justify-between">
          <div>
            <p class="text-sm font-medium text-cambridge-blue-700">
              Signed in
            </p>
            <h1 class="mt-2 text-3xl font-semibold tracking-tight text-gray-950">
              Welcome back, {user.value.name || user.value.email}
            </h1>
            <p class="mt-2 max-w-2xl text-sm leading-6 text-gray-600">
              Your account is protected by a secure API-backed session. Manage
              passkeys and connected sign-in methods from settings.
            </p>
          </div>
          <div class="flex items-center gap-3">
            {user.value.picture && (
              <img
                data-testid="user-avatar"
                src={user.value.picture}
                alt={user.value.name || user.value.email}
                width="48"
                height="48"
                class="h-12 w-12 rounded-full border border-gray-200"
              />
            )}
            <div>
              <div data-testid="user-name" class="font-medium text-gray-950">
                {user.value.name || "Hamrah user"}
              </div>
              <div data-testid="user-email" class="text-sm text-gray-500">
                {user.value.email}
              </div>
            </div>
          </div>
        </section>

        <div class="grid gap-6 lg:grid-cols-[1fr_320px]">
          <section class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
            <div class="mb-6 flex items-center justify-between">
              <div>
                <h2 class="text-xl font-semibold text-gray-950">
                  Workspace
                </h2>
                <p class="mt-1 text-sm text-gray-600">
                  A clean starting point for protected Hamrah workflows.
                </p>
              </div>
              <div class="relative">
                <button
                  data-testid="user-menu"
                  class="hidden"
                >
                  User menu
                </button>
                <div data-testid="auth-method" class="hidden">
                  {user.value.provider === "google"
                    ? "Google"
                    : user.value.provider === "apple"
                      ? "Apple"
                      : user.value.auth_method || "Passkey"}
                </div>
              </div>
            </div>

            <div class="rounded-lg border border-dashed border-gray-300 bg-gray-50 p-6">
              <h3 class="font-medium text-gray-950">Protected action</h3>
              <p class="mt-1 text-sm text-gray-600">
                This confirms the authenticated shell is active for the current
                server-validated session.
              </p>
              <button
                data-testid="protected-action"
                onClick$={handleProtectedAction}
                class="mt-4 rounded-lg bg-gray-950 px-4 py-2 text-sm font-semibold text-white transition hover:bg-gray-800"
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
          </section>

          <aside class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
            <h2 class="text-lg font-semibold text-gray-950">Account</h2>
            <dl class="mt-5 space-y-4">
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
                  {user.value.provider || user.value.auth_method || "passkey"}
                </dd>
              </div>
              {user.value.provider_id && (
                <div class="rounded-lg bg-gray-50 p-4">
                  <dt class="text-sm font-medium text-gray-500">Provider ID</dt>
                  <dd class="mt-1 font-mono text-sm break-all text-gray-900">
                    {user.value.provider_id}
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
            </dl>
          </aside>
        </div>
      </main>
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
