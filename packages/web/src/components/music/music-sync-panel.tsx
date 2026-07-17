import { $, component$, useSignal } from "@builder.io/qwik";
import type { MusicConnectionWire, MusicImportWire, MusicProvider } from "@hamrah/shared";
import { createApiClient } from "~/lib/auth/api-client";

const providers: MusicProvider[] = ["spotify", "tidal"];

type MusicSyncPanelProps = {
  initialConnections: MusicConnectionWire[];
  initialImports: MusicImportWire[];
  initialError?: string;
};

export const MusicSyncPanel = component$((props: MusicSyncPanelProps) => {
  const connections = useSignal(props.initialConnections);
  const imports = useSignal(props.initialImports);
  const error = useSignal<string | undefined>(props.initialError);
  const includeSaved = useSignal(false);

  const connect = $(async (provider: MusicProvider) => {
    error.value = undefined;
    try {
      const client = createApiClient();
      const result = await client.post<{ authorization_url: string }>(
        `/v1/music/connections/${provider}/authorize`, { redirect_path: "/settings" },
      );
      window.location.assign(result.authorization_url);
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : `Unable to connect ${provider}.`;
    }
  });

  const startImport = $(async () => {
    error.value = undefined;
    try {
      const client = createApiClient();
      const run = await client.post<MusicImportWire>("/v1/music/imports", {
        include_owned_playlists: true,
        include_saved_playlists: includeSaved.value,
        include_followed_artists: true,
      });
      imports.value = [run, ...imports.value];
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : "Unable to start import.";
    }
  });

  const connected = (provider: MusicProvider) => connections.value.some(
    (connection) => connection.provider === provider && connection.status === "connected",
  );

  return <section class="mt-6 border-t border-gray-200 pt-6">
    <h2 class="text-xl font-semibold text-gray-950">Music import</h2>
    <p class="mt-2 text-sm leading-6 text-gray-600">
      Import your Spotify playlists and followed artists into private TIDAL playlists.
    </p>
    {error.value && <p class="mt-3 rounded-md bg-red-50 p-3 text-sm text-red-700">{error.value}</p>}
    <div class="mt-4 grid gap-3 sm:grid-cols-2">
      {providers.map((provider) => <div key={provider} class="flex items-center justify-between rounded-lg border border-gray-200 p-4">
        <span class="font-medium capitalize text-gray-950">{provider}</span>
        {connected(provider) ? <span class="text-sm text-emerald-700">Connected</span> : <button class="rounded-lg border border-gray-300 px-3 py-2 text-sm font-semibold" onClick$={() => connect(provider)}>Connect</button>}
      </div>)}
    </div>
    <label class="mt-4 flex items-center gap-2 text-sm text-gray-700">
      <input type="checkbox" checked={includeSaved.value} onChange$={(event) => { includeSaved.value = (event.target as HTMLInputElement).checked; }} />
      Also import playlists saved in my Spotify library
    </label>
    <button class="mt-4 rounded-lg bg-gray-950 px-4 py-2 text-sm font-semibold text-white disabled:opacity-50" disabled={!connected("spotify") || !connected("tidal")} onClick$={startImport}>Start import</button>
    {imports.value[0] && <p class="mt-3 text-sm text-gray-600">Latest import: {imports.value[0].status} · {imports.value[0].imported_items} imported · {imports.value[0].unmatched_items} unmatched</p>}
  </section>;
});
