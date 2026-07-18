import { $, component$, useSignal, useVisibleTask$ } from "@builder.io/qwik";
import type {
  MusicConnectionWire,
  MusicImportWire,
  MusicProvider,
} from "@hamrah/shared";
import { createApiClient } from "~/lib/auth/api-client";

const providers: MusicProvider[] = ["spotify", "tidal"];

const isActiveImport = (run: MusicImportWire) =>
  run.status === "queued" || run.status === "running";

const canRestartImport = (run: MusicImportWire) =>
  run.status === "failed" || run.status === "partial";

export const failedImportMessage = (run: MusicImportWire) =>
  run.error ??
  "Restart the incomplete import to safely reuse its original TIDAL idempotency keys.";

export const musicImportOptionsDisabled = (run?: MusicImportWire) =>
  Boolean(run && isActiveImport(run));

export const importStage = (run: MusicImportWire) => {
  if (run.status === "failed") {
    switch (run.stage) {
      case "reading_spotify":
        return "Import failed while reading Spotify";
      case "creating_playlists":
        return "Import failed while creating TIDAL playlists";
      case "adding_playlist_tracks":
        return "Import failed while matching or adding playlist tracks";
      case "matching_artists":
      case "following_artists":
        return "Import failed while matching or following artists";
      case "saving_liked_tracks":
        return "Import failed while saving Liked Songs";
      default:
        return "Import failed";
    }
  }
  switch (run.stage) {
    case "queued":
    case "preparing":
      return "Preparing secure connections";
    case "reading_spotify":
      return "Reading selected Spotify collections";
    case "creating_playlists":
      return `Creating TIDAL playlists: ${run.playlists_imported} of ${run.playlist_total}`;
    case "adding_playlist_tracks":
      return `Adding playlist tracks: ${run.playlist_tracks_imported} of ${run.playlist_track_total}`;
    case "matching_artists":
      return `Checking artists for exact TIDAL matches: ${run.artists_checked} of ${run.artist_total}`;
    case "following_artists":
      return `Following exact TIDAL matches: ${run.artists_followed} of ${run.artists_matched}`;
    case "saving_liked_tracks":
      return `Saving Liked Songs: ${run.saved_tracks_imported} of ${run.saved_track_total}`;
    case "completed":
      return run.status === "partial"
        ? "Completed with unmatched items"
        : "Completed";
    case "failed":
      return "Import failed";
  }
};

type MusicSyncPanelProps = {
  initialConnections: MusicConnectionWire[];
  initialImports: MusicImportWire[];
  initialError?: string;
};

export const MusicSyncPanel = component$((props: MusicSyncPanelProps) => {
  const connections = useSignal(props.initialConnections);
  const imports = useSignal(props.initialImports);
  const error = useSignal<string | undefined>(props.initialError);
  const includeSaved = useSignal(
    props.initialImports[0]?.include_saved_playlists ?? false,
  );
  const includeSavedTracks = useSignal(
    props.initialImports[0]?.include_saved_tracks ?? false,
  );
  const isImporting = useSignal(false);

  // eslint-disable-next-line qwik/no-use-visible-task -- Polling must run in the signed-in browser session.
  useVisibleTask$(({ cleanup, track }) => {
    const latestImport = track(() => imports.value.at(0));
    if (!latestImport || !isActiveImport(latestImport)) return;

    const refresh = async () => {
      try {
        imports.value =
          await createApiClient().get<MusicImportWire[]>("/v1/music/imports");
      } catch {
        // Preserve the most recent status while a transient network request fails.
      }
    };
    const timer = window.setInterval(refresh, 2_000);
    cleanup(() => window.clearInterval(timer));
  });

  const connect = $(async (provider: MusicProvider) => {
    error.value = undefined;
    try {
      const client = createApiClient();
      const result = await client.post<{ authorization_url: string }>(
        `/v1/music/connections/${provider}/authorize`,
        { redirect_path: "/settings" },
      );
      window.location.assign(result.authorization_url);
    } catch (cause) {
      error.value =
        cause instanceof Error
          ? cause.message
          : `Unable to connect ${provider}.`;
    }
  });

  const disconnect = $(async (provider: MusicProvider) => {
    error.value = undefined;
    try {
      await createApiClient().delete(`/v1/music/connections/${provider}`);
      connections.value = connections.value.filter(
        (connection) => connection.provider !== provider,
      );
    } catch (cause) {
      error.value =
        cause instanceof Error
          ? cause.message
          : `Unable to disconnect ${provider}.`;
    }
  });

  const startImport = $(async () => {
    error.value = undefined;
    isImporting.value = true;
    const refreshTimer = window.setInterval(async () => {
      try {
        imports.value =
          await createApiClient().get<MusicImportWire[]>("/v1/music/imports");
      } catch {
        // The completed request will still surface its result or error below.
      }
    }, 500);
    try {
      const client = createApiClient();
      const run = await client.post<MusicImportWire>("/v1/music/imports", {
        include_owned_playlists: true,
        include_saved_playlists: includeSaved.value,
        include_followed_artists: true,
        include_saved_tracks: includeSavedTracks.value,
      });
      imports.value = [run, ...imports.value];
    } catch (cause) {
      try {
        const currentImports =
          await createApiClient().get<MusicImportWire[]>("/v1/music/imports");
        imports.value = currentImports;
        if (currentImports[0] && isActiveImport(currentImports[0])) return;
        if (currentImports[0] && canRestartImport(currentImports[0])) {
          error.value = failedImportMessage(currentImports[0]);
          return;
        }
      } catch {
        // Show the original start error when the status refresh also fails.
      }
      error.value =
        cause instanceof Error ? cause.message : "Unable to start import.";
    } finally {
      window.clearInterval(refreshTimer);
      isImporting.value = false;
    }
  });

  const restartImport = $(async (importId: string) => {
    error.value = undefined;
    isImporting.value = true;
    const refreshTimer = window.setInterval(async () => {
      try {
        imports.value =
          await createApiClient().get<MusicImportWire[]>("/v1/music/imports");
      } catch {
        // The completed request will still surface its result or error below.
      }
    }, 500);
    try {
      const run = await createApiClient().post<MusicImportWire>(
        `/v1/music/imports/${importId}/restart`,
        {
          include_owned_playlists: true,
          include_saved_playlists: includeSaved.value,
          include_followed_artists: true,
          include_saved_tracks: includeSavedTracks.value,
        },
      );
      imports.value = [
        run,
        ...imports.value.filter((item) => item.id !== run.id),
      ];
    } catch (cause) {
      error.value =
        cause instanceof Error ? cause.message : "Unable to restart import.";
    } finally {
      window.clearInterval(refreshTimer);
      isImporting.value = false;
    }
  });

  const connected = (provider: MusicProvider) =>
    connections.value.some(
      (connection) =>
        connection.provider === provider && connection.status === "connected",
    );
  const accountId = (provider: MusicProvider) =>
    connections.value.find(
      (connection) =>
        connection.provider === provider && connection.status === "connected",
    )?.provider_account_id;
  const accountName = (provider: MusicProvider) =>
    connections.value.find(
      (connection) =>
        connection.provider === provider && connection.status === "connected",
    )?.provider_account_name;

  return (
    <section class="mt-6 border-t border-gray-200 pt-6">
      <h2 class="text-xl font-semibold text-gray-950">Music import</h2>
      <p class="mt-2 text-sm leading-6 text-gray-600">
        Copy Spotify playlist tracks and Liked Songs to TIDAL when their ISRC
        identifiers match exactly, and follow exact artist-name matches. Public
        Spotify playlists remain public; all others are unlisted.
      </p>
      {error.value && (
        <p class="mt-3 rounded-md bg-red-50 p-3 text-sm text-red-700">
          {error.value}
        </p>
      )}
      <div class="mt-4 grid gap-3 sm:grid-cols-2">
        {providers.map((provider) => (
          <div
            key={provider}
            class="flex flex-col gap-3 rounded-lg border border-gray-200 p-4 sm:flex-row sm:items-center sm:justify-between"
          >
            <span class="font-medium capitalize text-gray-950">{provider}</span>
            {connected(provider) ? (
              <div class="flex min-w-0 items-center justify-between gap-3 sm:justify-end">
                <div class="min-w-0 text-left sm:text-right">
                  <p class="text-sm text-emerald-700">Connected</p>
                  {accountName(provider) && (
                    <p class="truncate text-sm font-medium text-gray-800">
                      {accountName(provider)}
                    </p>
                  )}
                  {accountId(provider) && (
                    <p class="text-xs text-gray-500">
                      {provider === "spotify" ? "Spotify" : "TIDAL"} ID:{" "}
                      {accountId(provider)}
                    </p>
                  )}
                </div>
                <div class="flex shrink-0 flex-col gap-1">
                  <button
                    class="rounded-lg border border-gray-300 px-3 py-2 text-sm font-semibold"
                    onClick$={() => connect(provider)}
                  >
                    Change account
                  </button>
                  <button
                    class="px-3 py-1 text-xs font-medium text-red-700"
                    onClick$={() => disconnect(provider)}
                  >
                    Disconnect
                  </button>
                </div>
              </div>
            ) : (
              <button
                class="rounded-lg border border-gray-300 px-3 py-2 text-sm font-semibold"
                onClick$={() => connect(provider)}
              >
                Connect
              </button>
            )}
          </div>
        ))}
      </div>
      <label class="mt-4 flex items-center gap-2 text-sm text-gray-700">
        <input
          type="checkbox"
          checked={includeSaved.value}
          disabled={musicImportOptionsDisabled(imports.value[0])}
          onChange$={(event) => {
            includeSaved.value = (event.target as HTMLInputElement).checked;
          }}
        />
        Also import playlists saved in my Spotify library
      </label>
      <label class="mt-3 flex items-center gap-2 text-sm text-gray-700">
        <input
          type="checkbox"
          checked={includeSavedTracks.value}
          disabled={musicImportOptionsDisabled(imports.value[0])}
          onChange$={(event) => {
            includeSavedTracks.value = (
              event.target as HTMLInputElement
            ).checked;
          }}
        />
        Also import Spotify Liked Songs
      </label>
      {includeSavedTracks.value && (
        <p class="mt-2 text-sm text-gray-600">
          Spotify will ask for permission to read your Liked Songs. Reconnect
          Spotify if requested.
        </p>
      )}
      <button
        class="mt-4 w-full rounded-lg bg-gray-950 px-4 py-2 text-sm font-semibold text-white transition active:scale-[0.99] disabled:cursor-wait disabled:opacity-50 sm:w-auto"
        disabled={
          isImporting.value ||
          Boolean(imports.value[0] && isActiveImport(imports.value[0])) ||
          !connected("spotify") ||
          !connected("tidal")
        }
        onClick$={() => {
          const latestImport = imports.value.at(0);
          return latestImport && canRestartImport(latestImport)
            ? restartImport(latestImport.id)
            : startImport();
        }}
      >
        {isImporting.value
          ? "Importing…"
          : imports.value[0] && isActiveImport(imports.value[0])
            ? "Import in progress"
            : imports.value[0]?.status === "failed"
              ? "Restart failed import"
              : imports.value[0]?.status === "partial"
                ? "Retry partial import"
                : "Start import"}
      </button>
      {imports.value[0] && (
        <div class="mt-3 rounded-md bg-gray-50 p-3 text-sm text-gray-600">
          <p class="font-medium text-gray-950">
            {importStage(imports.value[0])}
          </p>
          <p class="mt-1">
            Selected from Spotify: {imports.value[0].playlist_total} playlists ·{" "}
            {imports.value[0].playlist_track_total} playlist tracks ·{" "}
            {imports.value[0].saved_track_total} Liked Songs ·{" "}
            {imports.value[0].artist_total} followed artists.
          </p>
          <p class="mt-1">
            {imports.value[0].playlists_imported} playlists created ·{" "}
            {imports.value[0].playlist_tracks_imported} playlist tracks added ·{" "}
            {imports.value[0].saved_tracks_imported} Liked Songs saved ·{" "}
            {imports.value[0].artists_followed} artists followed ·{" "}
            {imports.value[0].unmatched_items} unmatched
          </p>
          {canRestartImport(imports.value[0]) && (
            <p class="mt-1">
              Restarting reuses this import's original TIDAL idempotency keys.
            </p>
          )}
          {imports.value[0].error && (
            <p class="mt-1 text-red-700">{imports.value[0].error}</p>
          )}
          <p class="mt-1 text-xs text-gray-500">
            Import reference: {imports.value[0].id.slice(0, 8)}
          </p>
        </div>
      )}
    </section>
  );
});
