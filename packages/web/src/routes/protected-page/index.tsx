import { component$ } from "@builder.io/qwik";
import type { DocumentHead } from "@builder.io/qwik-city";
import { useUserLoader } from "../layout";

export default component$(() => {
  const user = useUserLoader();

  return (
    <div class="min-h-screen bg-gray-50 py-8">
      <div class="mx-auto max-w-4xl px-4">
        <div class="rounded-lg bg-white p-6 shadow">
          <div class="mb-6 flex items-center justify-between">
            <h1 class="text-3xl font-bold text-gray-900">Protected Page</h1>
            <a
              href="/"
              class="rounded-md bg-gray-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-gray-700"
            >
              Back to Dashboard
            </a>
          </div>

          <div class="border-t pt-6">
            <div class="rounded-lg border border-blue-200 bg-blue-50 p-6">
              <h2 class="mb-2 text-xl font-semibold text-blue-900">
                🔒 This is a protected page
              </h2>
              <p class="text-blue-800">
                You can only see this page because you are authenticated as{" "}
                <strong>{user.value.name}</strong> ({user.value.email}).
              </p>
            </div>

            <div class="mt-6">
              <h3 class="mb-4 text-lg font-medium text-gray-900">
                Your Authentication Details
              </h3>
              <div class="rounded-lg bg-gray-50 p-4">
                <dl class="space-y-2">
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
                      {user.value.provider?.charAt(0).toUpperCase()}
                      {user.value.provider?.slice(1)}
                    </dd>
                  </div>
                  <div>
                    <dt class="text-sm font-medium text-gray-500">
                      Last Login:
                    </dt>
                    <dd class="text-sm text-gray-900">
                      {user.value.lastLoginAt
                        ? new Date(user.value.lastLoginAt).toLocaleString()
                        : "Current session"}
                    </dd>
                  </div>
                </dl>
              </div>
            </div>
          </div>
        </div>
      </div>
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
