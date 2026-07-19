import { component$ } from "@builder.io/qwik";
import { type DocumentHead } from "@builder.io/qwik-city";
import { useUserLoader } from "../layout";
import { PasskeyManagement } from "~/components/auth/passkey-management";
import { linkedAuthProviders } from "~/lib/auth/linked-providers";


export default component$(() => {
  const user = useUserLoader();
  const providers = linkedAuthProviders(user.value);

  return (
    <div class="min-h-screen">
      <header class="border-b border-gray-200 bg-white/90 backdrop-blur">
        <div class="mx-auto flex max-w-6xl items-center justify-between px-4 py-4 sm:px-6">
          <a
            href="/"
            class="text-lg font-semibold tracking-tight text-gray-950"
          >
            Hamrah
          </a>
          <nav class="flex items-center gap-2">
            <a
              href="/"
              class="rounded-lg px-3 py-2 text-sm font-medium text-gray-700 transition hover:bg-gray-100"
            >
              Dashboard
            </a>
          </nav>
        </div>
      </header>

      <main class="mx-auto max-w-6xl px-4 py-8 sm:px-6">
        <div class="mb-8">
          <p class="text-sm font-medium text-cambridge-blue-700">Account</p>
          <h1 class="mt-2 text-3xl font-semibold tracking-tight text-gray-950">
            Settings
          </h1>
          <p class="mt-2 max-w-2xl text-sm leading-6 text-gray-600">
            Manage your profile, connected sign-in methods, and passkeys.
          </p>
        </div>

        <div class="grid gap-6 lg:grid-cols-[320px_1fr]">
          <aside class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
            <h2 class="mb-4 text-lg font-semibold text-gray-950">
              Profile Information
            </h2>
            <div class="flex items-center space-x-4">
              {user.value.picture && (
                <img
                  src={user.value.picture}
                  alt={user.value.name || user.value.email}
                  width="64"
                  height="64"
                  class="h-16 w-16 rounded-full"
                />
              )}
              <div>
                <p class="text-lg font-medium text-gray-900">
                  {user.value.name || "Hamrah user"}
                </p>
                <p class="text-sm text-gray-600">{user.value.email}</p>
                {user.value.provider && (
                  <p class="mt-1 text-xs text-gray-500">
                    Connected with{" "}
                    {user.value.provider === "google"
                      ? "Google"
                      : user.value.provider === "apple"
                        ? "Apple"
                        : user.value.provider}
                  </p>
                )}
              </div>
            </div>
          </aside>

          <section class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
            <div>
              <h2 class="text-xl font-semibold text-gray-950">Security</h2>
              <p class="mt-2 text-sm leading-6 text-gray-600">
                Add passkeys for passwordless access and review connected
                sign-in methods.
              </p>
            </div>

            <div class="mt-6 space-y-3">
              <h3 class="text-sm font-semibold text-gray-950">
                Connected sign-in methods
              </h3>
              <div class="grid gap-3 sm:grid-cols-2">
                <div
                  class={[
                    "flex items-center justify-between rounded-lg border p-4",
                    providers.has("google")
                      ? "border-emerald-200 bg-emerald-50"
                      : "border-gray-200 bg-white",
                  ].join(" ")}
                >
                  <div class="flex min-w-0 items-center">
                    <svg class="mr-3 h-5 w-5 shrink-0" viewBox="0 0 24 24">
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
                    <div>
                      <p class="text-sm font-medium text-gray-950">Google</p>
                      {providers.has("google") && (
                        <p class="text-xs text-emerald-700">Connected</p>
                      )}
                    </div>
                  </div>
                  {!providers.has("google") && (
                    <a
                      href="/auth/google?redirect=%2Fsettings&link_provider=true"
                      class="rounded-lg border border-gray-200 px-3 py-2 text-sm font-semibold text-gray-800 transition hover:border-gray-300 hover:bg-gray-50"
                    >
                      Connect
                    </a>
                  )}
                </div>

                <div
                  class={[
                    "flex items-center justify-between rounded-lg border p-4",
                    providers.has("apple")
                      ? "border-emerald-200 bg-emerald-50"
                      : "border-gray-200 bg-white",
                  ].join(" ")}
                >
                  <div class="flex min-w-0 items-center">
                    <svg
                      class={[
                        "mr-3 h-5 w-5 shrink-0",
                        providers.has("apple")
                          ? "text-emerald-700"
                          : "text-gray-800",
                      ].join(" ")}
                      fill="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path d="M18.71 19.5c-.83 1.24-1.71 2.45-3.05 2.47-1.34.03-1.77-.79-3.29-.79-1.53 0-2 .77-3.27.82-1.31.05-2.3-1.32-3.14-2.53C4.25 17 2.94 12.45 4.7 9.39c.87-1.52 2.43-2.48 4.12-2.51 1.28-.02 2.5.87 3.29.87.78 0 2.26-1.07 3.81-.91.65.03 2.47.26 3.64 1.98-.09.06-2.17 1.28-2.15 3.81.03 3.02 2.65 4.03 2.68 4.04-.03.07-.42 1.44-1.38 2.83M13 3.5c.73-.83 1.94-1.46 2.94-1.5.13 1.17-.34 2.35-1.04 3.19-.69.85-1.83 1.51-2.95 1.42-.15-1.15.41-2.35 1.05-3.11z" />
                    </svg>
                    <div>
                      <p class="text-sm font-medium text-gray-950">Apple</p>
                      {providers.has("apple") && (
                        <p class="text-xs text-emerald-700">Connected</p>
                      )}
                    </div>
                  </div>
                  {!providers.has("apple") && (
                    <a
                      href="/auth/apple?redirect=%2Fsettings&link_provider=true"
                      class="rounded-lg border border-gray-200 px-3 py-2 text-sm font-semibold text-gray-800 transition hover:border-gray-300 hover:bg-gray-50"
                    >
                      Connect
                    </a>
                  )}
                </div>
              </div>
            </div>

            {/* Passkey Management */}
            <div class="mt-6 border-t border-gray-200 pt-6">
              <PasskeyManagement
                userId={user.value.id}
                userEmail={user.value.email}
              />
            </div>
            <div class="mt-6 border-t border-gray-200 pt-6">
              <h3 class="text-sm font-semibold text-gray-950">Music</h3>
              <p class="mt-1 text-sm text-gray-600">Manage Spotify and TIDAL connections, transfers, and unmatched songs.</p>
              <a href="/music" class="mt-3 inline-block rounded-lg border border-gray-300 px-3 py-2 text-sm font-semibold text-gray-800 hover:bg-gray-50">Open music management</a>
            </div>
          </section>
        </div>
      </main>
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
