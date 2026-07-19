import { component$ } from "@builder.io/qwik";
import { routeLoader$, type DocumentHead } from "@builder.io/qwik-city";
import type { MusicConnectionWire, MusicImportWire } from "@hamrah/shared";
import { MusicSyncPanel } from "~/components/music/music-sync-panel";
import { createApiClient } from "~/lib/auth/api-client";

export const useMusicLoader = routeLoader$(async (event) => {
  const client = createApiClient(event);
  const [connections, imports] = await Promise.allSettled([
    client.get<MusicConnectionWire[]>("/v1/music/connections"),
    client.get<MusicImportWire[]>("/v1/music/imports"),
  ]);
  const errors = [connections, imports]
    .filter((result): result is PromiseRejectedResult => result.status === "rejected")
    .map((result) =>
      result.reason instanceof Error ? result.reason.message : "A music request failed.",
    );
  return {
    connections: connections.status === "fulfilled" ? connections.value : [],
    imports: imports.status === "fulfilled" ? imports.value : [],
    error: errors.length
      ? `Some music data could not be refreshed: ${errors.join(" ")}`
      : undefined,
  };
});

export default component$(() => {
  const music = useMusicLoader();
  return (
    <div class="min-h-screen bg-gray-50">
      <header class="border-b border-gray-200 bg-white/90 backdrop-blur">
        <div class="mx-auto flex max-w-6xl items-center justify-between px-4 py-4 sm:px-6">
          <a href="/" class="text-lg font-semibold tracking-tight text-gray-950">Hamrah</a>
          <nav class="flex items-center gap-2">
            <a href="/" class="rounded-lg px-3 py-2 text-sm font-medium text-gray-700 hover:bg-gray-100">Dashboard</a>
            <a href="/settings" class="rounded-lg px-3 py-2 text-sm font-medium text-gray-700 hover:bg-gray-100">Settings</a>
          </nav>
        </div>
      </header>
      <main class="mx-auto max-w-4xl px-4 py-8 sm:px-6">
        <div class="mb-8">
          <p class="text-sm font-medium text-cambridge-blue-700">Library</p>
          <h1 class="mt-2 text-3xl font-semibold tracking-tight text-gray-950">Music</h1>
          <p class="mt-2 max-w-2xl text-sm leading-6 text-gray-600">Compare your Spotify selections with TIDAL, transfer exact matches, and review songs TIDAL could not support.</p>
        </div>
        <div class="rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
          <MusicSyncPanel initialConnections={music.value.connections} initialImports={music.value.imports} initialError={music.value.error} />
        </div>
      </main>
    </div>
  );
});

export const head: DocumentHead = { title: "Music - Hamrah" };
