import { component$ } from "@builder.io/qwik";
import type { DocumentHead } from "@builder.io/qwik-city";
import { useUserLoader } from "../layout";

export default component$(() => {
  const user = useUserLoader();

  return (
    <div class="min-h-screen">
      <header class="border-b border-gray-200 bg-white/90 backdrop-blur">
        <div class="mx-auto flex max-w-6xl items-center justify-between px-4 py-4 sm:px-6">
          <a href="/" class="text-lg font-semibold tracking-tight text-gray-950">
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

      <main class="mx-auto max-w-4xl px-4 py-8 sm:px-6">
        <section class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
          <p class="text-sm font-medium text-cambridge-blue-700">
            Protected route
          </p>
          <h1 class="mt-2 text-3xl font-semibold tracking-tight text-gray-950">
            Session verified
          </h1>
          <p class="mt-3 text-sm leading-6 text-gray-600">
            You can only see this page because you are authenticated as{" "}
            <strong>{user.value.name || user.value.email}</strong>.
          </p>

          <div class="mt-6 rounded-lg bg-gray-50 p-4">
            <h2 class="mb-4 text-lg font-medium text-gray-950">
              Your Authentication Details
            </h2>
            <dl class="grid gap-4 sm:grid-cols-2">
              <div>
                <dt class="text-sm font-medium text-gray-500">User ID:</dt>
                <dd class="font-mono text-sm text-gray-900">
                  {user.value.id}
                </dd>
              </div>
              <div>
                <dt class="text-sm font-medium text-gray-500">Email:</dt>
                <dd class="text-sm text-gray-900">{user.value.email}</dd>
              </div>
              <div>
                <dt class="text-sm font-medium text-gray-500">
                  Authentication Method:
                </dt>
                <dd class="text-sm text-gray-900">
                  {user.value.provider || user.value.auth_method || "passkey"}
                </dd>
              </div>
              <div>
                <dt class="text-sm font-medium text-gray-500">Last Login:</dt>
                <dd class="text-sm text-gray-900">
                  {user.value.last_login_at
                    ? new Date(user.value.last_login_at).toLocaleString()
                    : "Current session"}
                </dd>
              </div>
            </dl>
          </div>
        </section>
      </main>
    </div>
  );
});

export const head: DocumentHead = {
  title: "Protected Page - Hamrah App",
  meta: [
    {
      name: "description",
      content: "This page requires authentication to access",
    },
  ],
};
