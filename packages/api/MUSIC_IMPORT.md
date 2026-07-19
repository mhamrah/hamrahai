# Spotify and TIDAL reconciliation

## Product contract

Hamrah performs an additive, user-authorized reconciliation between Spotify and
TIDAL.

- Owned Spotify playlists participate by default. The user can also include
  playlists saved in their Spotify library.
- When owned TIDAL playlists are included, a Spotify playlist and every TIDAL
  playlist with the same case- and whitespace-insensitive name form one
  reconciliation group.
- Hamrah treats same-name TIDAL playlists as duplicate copies from earlier
  Hamrah runs. It merges the union of their songs into one surviving TIDAL
  playlist, reconciles that union with Spotify, and only then removes the extra
  TIDAL playlists.
- A saved Spotify-to-TIDAL mapping is preferred only while that TIDAL playlist
  still exists. A stale mapping falls back to a present same-name playlist or a
  newly created playlist.
- Songs present only in Spotify are added to the surviving TIDAL playlist.
  Songs present only in the TIDAL group are added to the matching Spotify
  playlist.
- A TIDAL playlist without a Spotify counterpart creates a private Spotify
  playlist. Its same-name TIDAL copies are consolidated before removal.
- Reconciliation is additive. Removing a song from either provider does not
  remove it from the other provider.
- Cross-provider song matches prefer an exact normalized ISRC. When Spotify and
  TIDAL use different ISRCs for the same release, Spotify-to-TIDAL matching
  falls back to an exact case- and whitespace-insensitive title and primary
  artist match. TIDAL-to-Spotify matching remains ISRC-only.
- A TIDAL track write reported as `ALREADY_PRESENT` is a reconciled no-op, not
  an unmatched song. Other skipped TIDAL writes remain unmatched.
- Duplicate TIDAL content is copied directly by TIDAL track ID before a
  playlist is deleted. If TIDAL skips any of those writes, the run fails safely
  and keeps the duplicate playlists.
- Spotify Liked Songs are always reconciled additively in both directions.
  Followed artists continue to flow from Spotify to an exact-name TIDAL match.
- Artwork and playlist descriptions are not transferred.
- A newly created TIDAL playlist is public when its Spotify source is public
  and unlisted otherwise.

## Architecture

`src/music.rs` owns OAuth, encrypted token storage, import persistence, Cloud
Tasks scheduling, worker leases, and API routes. `src/music/import.rs` owns the
provider contracts and deterministic reconciliation behavior.

`POST /v1/music/imports` creates a queued `music_import_runs` row and enqueues a
Cloud Task. `POST /internal/music-imports/{id}/execute` claims the run and
executes it. Clients poll `GET /v1/music/imports` and
`GET /v1/music/imports/{id}/activity`.

The stages are:

1. `preparing`
2. `reading_spotify`
3. `creating_playlists`
4. `reconciling_tidal_playlists`
5. `adding_playlist_tracks`
6. `matching_artists`
7. `following_artists`
8. `saving_liked_tracks`
9. `completed` or `failed`

Each run:

1. Loads or refreshes both encrypted provider tokens.
2. Reads selected Spotify collections and owned TIDAL playlists.
3. Reads every selected playlist's tracks.
4. Groups same-name playlists, chooses a present canonical TIDAL playlist, and
   copies all TIDAL duplicate content into it.
5. Adds matched Spotify-only songs to TIDAL and exact-ISRC TIDAL-only songs to
   Spotify.
6. Persists the Spotify/TIDAL mapping and reconciled content hash.
7. Deletes extra same-name TIDAL playlists only after their content is safe in
   the canonical playlist.
8. Reconciles Liked Songs and followed artists.
9. Persists a completed, partial, or failed result plus unmatched-song details.

An inaccessible Spotify playlist is skipped and recorded as unmatched without
stopping other collections.

## Durable execution and rate limits

Cloud Run and Cloud Tasks both allow 30 minutes for the worker request. The
worker uses a 28-minute internal budget so it can checkpoint and enqueue a new
task before the request deadline.

The worker:

- records the Cloud Task name as its lease owner;
- updates a database heartbeat every minute;
- rejects an overlapping retry while the current lease is fresh;
- lets the same Cloud Task reclaim the run after a three-minute stale lease;
- schedules a new task and records `next_attempt_at` before acknowledging a
  long provider backoff; and
- treats a future scheduled retry as active instead of failing it under the
  normal five-minute stale-run check.

Cloud Tasks removes a task after any 2xx worker response. The handler therefore
returns 2xx only after the run reaches a terminal state or a successor task is
durably scheduled. A transient scheduling failure returns 5xx so Cloud Tasks
retains and retries the current task.

Spotify and TIDAL requests retry short `429 Too Many Requests` waits inline.
A `Retry-After` longer than 15 seconds is converted into a scheduled Cloud Task
instead of sleeping inside Cloud Run. Catalog matches are cached after each
lookup so a later rate limit does not discard completed work. A successful
TIDAL metadata fallback is cached against the source ISRC for later playlists
and retries. TIDAL write retries retain stable import-specific idempotency keys.
Artist-follow payloads are sorted, and their keys include a payload fingerprint,
so a restart cannot reuse a key with differently ordered artist IDs.

## Restart and data safety

Only one queued or running import is allowed per Hamrah user. A failed or
partial import must be restarted rather than replaced with a new run.

Restarting reuses:

- the import ID and TIDAL idempotency keys;
- per-import Spotify/TIDAL playlist mappings; and
- the user's last successful playlist mappings.

Mapping upserts repair stale IDs when a surviving TIDAL playlist changes.
Provider writes are additive, and every retry re-reads current provider content
before calculating missing songs. This makes a repair run safe for an account
that already contains playlists or tracks from incomplete prior runs.

OAuth completion verifies the provider's current-user endpoint before replacing
a stored connection. Provider tokens remain encrypted at rest and are never
returned to clients.

## Required configuration

Secret Manager secrets:

- `SPOTIFY_CLIENT_SECRET`
- `MUSIC_TOKEN_ENCRYPTION_KEY` — 32 random bytes, base64url encoded
- `MUSIC_IMPORT_TASK_SECRET` — random worker-authentication value

Repository variables:

- `SPOTIFY_CLIENT_ID`
- `TIDAL_CLIENT_ID`

Cloud Run runtime values:

- `GOOGLE_CLOUD_PROJECT`
- `GOOGLE_CLOUD_REGION`
- `MUSIC_IMPORT_TASK_QUEUE`
- `MUSIC_IMPORT_TASK_BASE_URL`
- `MUSIC_IMPORT_TASK_SERVICE_ACCOUNT`

Provider callbacks:

- `https://api.hamrah.app/v1/music/connections/spotify/callback`
- `https://api.hamrah.app/v1/music/connections/tidal/callback`

Spotify needs playlist read/write, followed-artist read, and library read/write
scopes. TIDAL needs playlist, collection, search, and user read/write scopes.
Reconnect an existing provider account after a required scope changes.

## Current-account repair check

Use an allowlisted Spotify account and the intended TIDAL account.

1. Connect or reconnect both accounts and confirm their displayed account names.
2. In TIDAL, leave all same-name playlists and their distinct songs in place.
3. In Spotify, leave the matching playlists and any Spotify-only songs in place.
4. Start a sync with owned playlists selected.
5. Verify each name has one TIDAL playlist containing the union of all former
   TIDAL copies plus matched Spotify-only songs.
6. Verify the matching Spotify playlist contains exact TIDAL-only matches.
7. Verify TIDAL-only playlist names now have a Spotify counterpart.
8. Review unmatched songs; their original provider content must still exist.
9. Run sync again and verify it adds no new playlist copies.

## Deliberately not implemented

This is a user-triggered reconciliation, not a continuously scheduled sync. It
does not propagate deletions, use fuzzy song matching, copy artwork or
descriptions, or infer that same-name TIDAL playlists were created by another
application. The metadata fallback requires both an exact normalized title and
an exact normalized primary artist.
