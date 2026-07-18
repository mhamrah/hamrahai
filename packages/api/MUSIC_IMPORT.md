# Spotify to TIDAL import

## Product contract

Hamrah provides a one-way, user-authorized import from Spotify to TIDAL.

- Spotify is a read-only source. Hamrah never writes to Spotify.
- TIDAL is the destination. Hamrah creates playlist shells and follows artists.
- No tracks, playlist items, artwork, or playlist descriptions are transferred.
- A public Spotify playlist creates a public TIDAL playlist. Every other Spotify
  playlist creates an unlisted TIDAL playlist, because TIDAL's third-party API
  does not expose a private playlist access type.
- Owned Spotify playlists are selected by default. The user can also select
  playlists saved in their Spotify library.
- An artist is followed only when TIDAL search returns an exact name match
  (case and whitespace insensitive). Ambiguous or absent matches are reported
  as unmatched; Hamrah never guesses.
- TIDAL collection data is never enumerated or exported.

## Architecture

`src/music.rs` owns OAuth, encrypted token storage, token refresh, API routes,
and import-run persistence. `src/music/import.rs` owns provider requests and
the deterministic transfer behavior.

`POST /v1/music/imports` performs one bounded import request synchronously.
While it runs, `GET /v1/music/imports` exposes the active run's stage and
collection-specific counts, so clients can show what is selected and its live
progress:

- selected Spotify playlists and followed artists;
- TIDAL playlists created;
- Spotify artists checked for an exact TIDAL match; and
- exact matches successfully followed.

The stages are `preparing`, `reading_spotify`, `creating_playlists`,
`matching_artists`, and `following_artists`, followed by a terminal status.

The import itself performs these steps:

1. Load or refresh each encrypted provider token.
2. Read the selected Spotify playlist metadata and followed artists.
3. Create empty TIDAL playlists with the source visibility mapping.
4. Search TIDAL for exact artist-name matches and follow matches in batches of
   at most 50.
5. Persist progress and a completed, partial, or failed `music_import_runs` row.

TIDAL write requests use an import-run-specific idempotency key. The database
also permits only one queued or running import per Hamrah user.

TIDAL API requests are serialized at one request per second. If TIDAL responds
with `429 Too Many Requests`, the importer respects its `Retry-After` value
and retries up to three times; write retries retain the original idempotency
key.

## Failures and restart safety

If an import fails or completes partially, clients show a retry action. Retrying
reuses the original import run and its TIDAL idempotency keys, rather than
creating a second run. This safely replays incomplete work without creating a
duplicate TIDAL playlist or artist relationship.

The API updates a progress heartbeat whenever it persists import progress. A
run that stops reporting for five minutes is marked failed with a restartable
error. New imports are blocked while a failed or partial import is available,
so a user must safely restart it before beginning another transfer.

## Required configuration

The deployment workflow configures public runtime values. The API service also
needs these Secret Manager secrets:

- `SPOTIFY_CLIENT_SECRET`
- `MUSIC_TOKEN_ENCRYPTION_KEY` — 32 random bytes, base64url encoded

Repository variables:

- `SPOTIFY_CLIENT_ID`
- `TIDAL_CLIENT_ID`

The exact callbacks are:

- `https://api.hamrah.app/v1/music/connections/spotify/callback`
- `https://api.hamrah.app/v1/music/connections/tidal/callback`

TIDAL connections request `playlists.write`, `collection.write`, `search.read`,
and `user.read`. Existing TIDAL connections must be reconnected after a scope
change.

## Manual integration check

Use an allowlisted Spotify development-mode account and a TIDAL test account.

1. In Hamrah Settings, connect (or reconnect) Spotify and TIDAL.
2. Create one public and one non-public Spotify playlist; add a followed artist
   with an unambiguous TIDAL name.
3. Start an import without saved playlists, then verify two empty TIDAL
   playlists with the correct public/unlisted visibility and the matched TIDAL
   artist follow.
4. Save another Spotify playlist, repeat with the saved-playlists option, and
   verify that its empty playlist is created.
5. Verify no tracks or playlist items were copied and that unmatched artists
   appear in the import's `unmatched_items` count.

## Deliberately not implemented

This is an import, not a recurring sync. It does not transfer tracks, delete
destination data, export TIDAL data, make heuristic artist matches, or run a
background job. If imports outgrow the request timeout, add a durable job
runner that preserves the same per-collection progress before increasing scope.
