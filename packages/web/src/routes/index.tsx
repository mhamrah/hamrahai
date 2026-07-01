import { $, component$ } from "@builder.io/qwik";
import {
  Form,
  routeAction$,
  routeLoader$,
  type DocumentHead,
} from "@builder.io/qwik-city";
import { authService } from "~/lib/auth/auth-service";
import {
  archiveLink,
  deleteLink,
  listLinks,
  type ServerLink,
} from "~/lib/links/links-api";
import { useUserLoader } from "./layout";

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "Unable to update links";
}

function linkTitle(link: ServerLink): string {
  return link.title?.trim() || link.canonical_url || link.original_url;
}

function linkHost(link: ServerLink): string {
  try {
    return new URL(link.canonical_url || link.original_url).host;
  } catch {
    return link.canonical_url || link.original_url;
  }
}

function formatDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "Unknown date";
  return new Intl.DateTimeFormat("en", {
    month: "short",
    day: "numeric",
    year: "numeric",
  }).format(date);
}

export const useLinksLoader = routeLoader$(async (event) => {
  try {
    return {
      links: await listLinks(event),
      error: null,
    };
  } catch (error) {
    return {
      links: [],
      error: errorMessage(error),
    };
  }
});

export const useArchiveLinkAction = routeAction$(async (data, event) => {
  const link_id = String(data.link_id || "").trim();
  if (!link_id) return { success: false, error: "Missing link_id" };

  try {
    await archiveLink(link_id, event);
    return { success: true };
  } catch (error) {
    return { success: false, error: errorMessage(error) };
  }
});

export const useDeleteLinkAction = routeAction$(async (data, event) => {
  const link_id = String(data.link_id || "").trim();
  if (!link_id) return { success: false, error: "Missing link_id" };

  try {
    await deleteLink(link_id, event);
    return { success: true };
  } catch (error) {
    return { success: false, error: errorMessage(error) };
  }
});

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
  const links = useLinksLoader();
  const archiveAction = useArchiveLinkAction();
  const deleteAction = useDeleteLinkAction();

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
            <p class="text-sm font-medium text-cambridge-blue-700">Signed in</p>
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
          <section class="rounded-lg border border-gray-200 bg-white shadow-sm">
            <div class="mb-6 flex items-center justify-between px-6 pt-6">
              <div>
                <h2 class="text-xl font-semibold text-gray-950">Inbox</h2>
                <p class="mt-1 text-sm text-gray-600">
                  Saved links synced from your devices.
                </p>
              </div>
              <div data-testid="auth-method" class="hidden">
                {user.value.provider === "google"
                  ? "Google"
                  : user.value.provider === "apple"
                    ? "Apple"
                    : user.value.auth_method || "Passkey"}
              </div>
            </div>

            {links.value.error && (
              <div class="mx-6 mb-4 rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700">
                {links.value.error}
              </div>
            )}

            {(archiveAction.value?.error || deleteAction.value?.error) && (
              <div class="mx-6 mb-4 rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700">
                {archiveAction.value?.error || deleteAction.value?.error}
              </div>
            )}

            {links.value.links.length === 0 ? (
              <div class="border-t border-gray-100 px-6 py-12 text-center">
                <h3 class="text-sm font-semibold text-gray-950">
                  No saved links
                </h3>
                <p class="mt-1 text-sm text-gray-600">
                  Share links from iOS or save them from another Hamrah client.
                </p>
              </div>
            ) : (
              <ul class="divide-y divide-gray-100 border-t border-gray-100">
                {links.value.links.map((link) => (
                  <li
                    key={link.id}
                    data-testid="link-row"
                    class="grid gap-4 px-6 py-4 sm:grid-cols-[1fr_auto] sm:items-center"
                  >
                    <a
                      href={link.canonical_url || link.original_url}
                      target="_blank"
                      rel="noreferrer"
                      class="min-w-0"
                    >
                      <div class="flex items-center gap-2">
                        <span class="h-2.5 w-2.5 shrink-0 rounded-full bg-emerald-500" />
                        <h3 class="truncate text-sm font-semibold text-gray-950">
                          {linkTitle(link)}
                        </h3>
                      </div>
                      <div class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-gray-500">
                        <span>{linkHost(link)}</span>
                        <span aria-hidden="true">·</span>
                        <span>{formatDate(link.created_at)}</span>
                        {link.save_count > 1 && (
                          <>
                            <span aria-hidden="true">·</span>
                            <span>{link.save_count} saves</span>
                          </>
                        )}
                      </div>
                      {(link.summary_short || link.snippet) && (
                        <p class="mt-2 line-clamp-2 text-sm leading-6 text-gray-600">
                          {link.summary_short || link.snippet}
                        </p>
                      )}
                    </a>

                    <div class="flex items-center gap-2">
                      <Form action={archiveAction}>
                        <input type="hidden" name="link_id" value={link.id} />
                        <button
                          type="submit"
                          data-testid="archive-link"
                          class="rounded-lg border border-gray-200 px-3 py-2 text-sm font-semibold text-gray-700 transition hover:border-gray-300 hover:bg-gray-50"
                        >
                          Archive
                        </button>
                      </Form>
                      <Form action={deleteAction}>
                        <input type="hidden" name="link_id" value={link.id} />
                        <button
                          type="submit"
                          data-testid="delete-link"
                          class="rounded-lg border border-red-200 px-3 py-2 text-sm font-semibold text-red-700 transition hover:border-red-300 hover:bg-red-50"
                        >
                          Delete
                        </button>
                      </Form>
                    </div>
                  </li>
                ))}
              </ul>
            )}
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
