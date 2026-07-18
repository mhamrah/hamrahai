import { describe, expect, it } from "vitest";
import type { MusicImportWire } from "@hamrah/shared";

import {
  failedImportMessage,
  musicImportOptionsDisabled,
} from "./music-sync-panel";

const failedImport = (error?: string): MusicImportWire => ({
  id: "import-1",
  status: "failed",
  include_owned_playlists: true,
  include_saved_playlists: false,
  include_followed_artists: true,
  include_saved_tracks: false,
  stage: "failed",
  total_items: 0,
  imported_items: 0,
  unmatched_items: 0,
  playlist_total: 0,
  playlists_imported: 0,
  artist_total: 0,
  artists_checked: 0,
  artists_matched: 0,
  artists_followed: 0,
  playlist_track_total: 0,
  playlist_tracks_imported: 0,
  saved_track_total: 0,
  saved_tracks_imported: 0,
  tracks_matched: 0,
  error,
  created_at: "2026-07-17T00:00:00Z",
});

describe("failedImportMessage", () => {
  it("preserves the actionable server reason", () => {
    expect(
      failedImportMessage(
        failedImport(
          "Spotify authorization needs user-library-read; reconnect Spotify to continue",
        ),
      ),
    ).toBe(
      "Spotify authorization needs user-library-read; reconnect Spotify to continue",
    );
  });

  it("falls back to safe restart guidance", () => {
    expect(failedImportMessage(failedImport())).toBe(
      "Restart the incomplete import to safely reuse its original TIDAL idempotency keys.",
    );
  });
});

describe("musicImportOptionsDisabled", () => {
  it("allows collection choices to change before retrying a failed import", () => {
    expect(musicImportOptionsDisabled(failedImport())).toBe(false);
  });

  it("locks collection choices while an import is active", () => {
    expect(
      musicImportOptionsDisabled({
        ...failedImport(),
        status: "running",
        stage: "reading_spotify",
      }),
    ).toBe(true);
  });
});
