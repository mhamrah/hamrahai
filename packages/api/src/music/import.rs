use std::{
    collections::{HashMap, HashSet},
    future::Future,
    sync::OnceLock,
    time::{Duration, Instant},
};

use reqwest::{Client, Response, StatusCode};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::{sync::Mutex, time::sleep};
use uuid::Uuid;

use crate::db::DbPool;

use super::query_value;

const SPOTIFY_API_BASE: &str = "https://api.spotify.com";
const TIDAL_API_BASE: &str = "https://openapi.tidal.com/v2";
const TIDAL_MEDIA_TYPE: &str = "application/vnd.api+json";
const TIDAL_REQUEST_INTERVAL: Duration = Duration::from_secs(1);
const TIDAL_RATE_LIMIT_RETRIES: u8 = 3;
const SPOTIFY_RATE_LIMIT_RETRIES: u8 = 3;
const MAX_INLINE_RATE_LIMIT_DELAY: Duration = Duration::from_secs(15);
const DEFERRED_RATE_LIMIT_PREFIX: &str = "hamrah_provider_rate_limit:";
const TIDAL_ISRC_FILTER_LIMIT: usize = 20;
const TIDAL_DUPLICATE_COLLECTION_ITEMS: &str = "DUPLICATE_ITEMS_IN_COLLECTION";
const TIDAL_ALREADY_PRESENT: &str = "ALREADY_PRESENT";
const INACCESSIBLE_SPOTIFY_PLAYLIST_WARNING: &str = "Some Spotify playlists could not be read. Check that you own or collaborate on them, then restart to retry safely.";

static TIDAL_NEXT_REQUEST_AT: OnceLock<Mutex<Instant>> = OnceLock::new();

fn tidal_next_request_at() -> &'static Mutex<Instant> {
    TIDAL_NEXT_REQUEST_AT.get_or_init(|| Mutex::new(Instant::now()))
}

async fn wait_for_tidal_request_slot() {
    let delay = {
        let mut next_request_at = tidal_next_request_at().lock().await;
        let now = Instant::now();
        let request_at = (*next_request_at).max(now);
        *next_request_at = request_at + TIDAL_REQUEST_INTERVAL;
        request_at.saturating_duration_since(now)
    };
    if !delay.is_zero() {
        sleep(delay).await;
    }
}

async fn defer_tidal_requests(delay: Duration) {
    let mut next_request_at = tidal_next_request_at().lock().await;
    *next_request_at = (*next_request_at).max(Instant::now() + delay);
}

fn retry_after(response: &Response, retry: u8) -> Duration {
    response
        .headers()
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(1_u64 << retry))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SpotifyPlaylist {
    pub id: String,
    pub name: String,
    pub owner_id: String,
    pub is_public: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SpotifyArtist {
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SpotifyTrack {
    pub source_id: Option<String>,
    pub isrc: Option<String>,
    pub name: String,
    pub artist_name: Option<String>,
    pub album_name: Option<String>,
}

#[derive(Default)]
pub(super) struct PlaylistMappings {
    pub(super) tidal_by_spotify: HashMap<String, String>,
    pub(super) current_import_spotify: HashSet<String>,
}

fn deferred_rate_limit_error(provider: &str, delay: Duration) -> String {
    format!(
        "{DEFERRED_RATE_LIMIT_PREFIX}{provider}:{}",
        delay.as_secs().max(1)
    )
}

pub(super) fn deferred_rate_limit_delay(message: &str) -> Option<Duration> {
    let (_, seconds) = message
        .strip_prefix(DEFERRED_RATE_LIMIT_PREFIX)?
        .split_once(':')?;
    seconds.parse().ok().map(Duration::from_secs)
}

pub(super) fn playlist_content_hash(tracks: &[SpotifyTrack]) -> String {
    let mut hasher = Sha256::new();
    for track in tracks {
        hasher.update(track.isrc.as_deref().unwrap_or("missing-isrc").as_bytes());
        hasher.update([0]);
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn unique_reconciliation_tracks(
    tracks: impl IntoIterator<Item = SpotifyTrack>,
) -> Vec<SpotifyTrack> {
    let mut seen = HashSet::new();
    tracks
        .into_iter()
        .filter(|track| {
            let key = if let Some(isrc) = &track.isrc {
                format!("isrc:{isrc}")
            } else if let Some(source_id) = &track.source_id {
                format!("source:{source_id}")
            } else {
                format!(
                    "metadata:{}:{}:{}",
                    track.name,
                    track.artist_name.as_deref().unwrap_or_default(),
                    track.album_name.as_deref().unwrap_or_default()
                )
            };
            seen.insert(key)
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TidalPlaylist {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TidalSavedTrack {
    pub isrc: Option<String>,
    pub name: String,
    pub artist_name: Option<String>,
    pub album_name: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TidalPlaylistVisibility {
    Public,
    Unlisted,
}

impl TidalPlaylistVisibility {
    fn as_api_value(self) -> &'static str {
        match self {
            Self::Public => "PUBLIC",
            Self::Unlisted => "UNLISTED",
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct ImportOptions {
    pub include_owned_playlists: bool,
    pub include_saved_playlists: bool,
    pub include_followed_artists: bool,
    pub include_saved_tracks: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ImportOutcome {
    pub total_items: i32,
    pub imported_items: i32,
    pub unmatched_items: i32,
    pub warning: Option<String>,
    pub unmatched_tracks: Vec<UnmatchedTrack>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct UnmatchedTrack {
    pub source_collection: String,
    pub track: SpotifyTrack,
    pub reason: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ImportProgress {
    pub stage: &'static str,
    pub activity: String,
    pub playlist_total: i32,
    pub playlists_imported: i32,
    pub artist_total: i32,
    pub artists_checked: i32,
    pub artists_matched: i32,
    pub artists_followed: i32,
    pub artists_unmatched: i32,
    pub playlist_track_total: i32,
    pub playlist_tracks_imported: i32,
    pub saved_track_total: i32,
    pub saved_tracks_imported: i32,
    pub tracks_matched: i32,
    pub tracks_unmatched: i32,
}

impl Default for ImportProgress {
    fn default() -> Self {
        Self {
            stage: "preparing",
            activity: "Preparing secure connections".to_string(),
            playlist_total: 0,
            playlists_imported: 0,
            artist_total: 0,
            artists_checked: 0,
            artists_matched: 0,
            artists_followed: 0,
            artists_unmatched: 0,
            playlist_track_total: 0,
            playlist_tracks_imported: 0,
            saved_track_total: 0,
            saved_tracks_imported: 0,
            tracks_matched: 0,
            tracks_unmatched: 0,
        }
    }
}

#[derive(Debug)]
pub(super) struct ImportFailure {
    pub message: String,
    pub outcome: ImportOutcome,
    pub progress: ImportProgress,
}

pub(super) trait MusicImportProvider {
    async fn spotify_current_user_id(&self) -> Result<String, String>;
    async fn spotify_playlists(&self) -> Result<Vec<SpotifyPlaylist>, String>;
    async fn spotify_playlist_tracks(&self, playlist_id: &str)
    -> Result<Vec<SpotifyTrack>, String>;
    async fn spotify_saved_tracks(&self) -> Result<Vec<SpotifyTrack>, String>;
    async fn spotify_tracks_by_isrc(
        &self,
        isrcs: &[String],
    ) -> Result<HashMap<String, String>, String>;
    async fn save_spotify_tracks(&self, track_ids: &[String]) -> Result<(), String>;
    async fn create_spotify_playlist(&self, playlist: &TidalPlaylist) -> Result<String, String>;
    async fn add_spotify_playlist_tracks(
        &self,
        playlist_id: &str,
        track_ids: &[String],
    ) -> Result<(), String>;
    async fn spotify_followed_artists(&self) -> Result<Vec<SpotifyArtist>, String>;
    async fn create_tidal_playlist(
        &self,
        playlist: &SpotifyPlaylist,
        visibility: TidalPlaylistVisibility,
        idempotency_key: String,
    ) -> Result<String, String>;
    async fn tidal_owned_playlists(&self) -> Result<Vec<TidalPlaylist>, String>;
    async fn tidal_playlist_tracks(&self, playlist_id: &str) -> Result<Vec<SpotifyTrack>, String>;
    async fn delete_tidal_playlist(
        &self,
        playlist_id: &str,
        idempotency_key: String,
    ) -> Result<(), String>;
    async fn tidal_tracks_by_isrc(
        &self,
        isrcs: &[String],
    ) -> Result<HashMap<String, String>, String>;
    async fn find_tidal_track(&self, track: &SpotifyTrack) -> Result<Option<String>, String>;
    async fn tidal_saved_tracks(&self) -> Result<Vec<TidalSavedTrack>, String>;
    #[allow(dead_code)]
    async fn add_tidal_playlist_tracks(
        &self,
        playlist_id: &str,
        track_ids: &[String],
        idempotency_key: String,
    ) -> Result<TrackWriteOutcome, String>;
    async fn save_tidal_tracks(
        &self,
        track_ids: &[String],
        idempotency_key: String,
    ) -> Result<TrackWriteOutcome, String>;
    async fn find_tidal_artist(&self, name: &str) -> Result<Option<String>, String>;
    async fn follow_tidal_artists(
        &self,
        artist_ids: &[String],
        idempotency_key: String,
    ) -> Result<FollowOutcome, String>;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct FollowOutcome {
    pub imported_items: i32,
    pub unmatched_items: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct TrackWriteOutcome {
    pub imported_items: i32,
    pub unmatched_items: i32,
}

pub(super) struct HttpMusicImportProvider {
    client: Client,
    spotify_access_token: String,
    tidal_access_token: String,
    spotify_api_base: String,
    tidal_api_base: String,
    cache_pool: Option<DbPool>,
}

impl HttpMusicImportProvider {
    fn client() -> Client {
        Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
            .build()
            .expect("music import HTTP client configuration is valid")
    }

    pub(super) fn new(
        spotify_access_token: String,
        tidal_access_token: String,
        cache_pool: DbPool,
    ) -> Self {
        Self {
            client: Self::client(),
            spotify_access_token,
            tidal_access_token,
            spotify_api_base: SPOTIFY_API_BASE.to_string(),
            tidal_api_base: TIDAL_API_BASE.to_string(),
            cache_pool: Some(cache_pool),
        }
    }

    #[cfg(test)]
    fn with_tidal_api_base(tidal_api_base: String) -> Self {
        Self {
            client: Self::client(),
            spotify_access_token: "spotify-access-token".to_string(),
            tidal_access_token: "tidal-access-token".to_string(),
            spotify_api_base: SPOTIFY_API_BASE.to_string(),
            tidal_api_base,
            cache_pool: None,
        }
    }

    #[cfg(test)]
    fn with_spotify_api_base(spotify_api_base: String) -> Self {
        Self {
            client: Self::client(),
            spotify_access_token: "spotify-access-token".to_string(),
            tidal_access_token: "tidal-access-token".to_string(),
            spotify_api_base,
            tidal_api_base: "http://unused".to_string(),
            cache_pool: None,
        }
    }

    async fn cached_track_matches(
        &self,
        provider: &str,
        isrcs: &[String],
    ) -> HashMap<String, Option<String>> {
        let Some(pool) = &self.cache_pool else {
            return HashMap::new();
        };
        match sqlx::query_as::<_, (String, Option<String>)>("SELECT isrc,target_track_id FROM music_catalog_track_mappings WHERE provider = $1 AND isrc = ANY($2) AND expires_at > NOW()")
            .bind(provider)
            .bind(isrcs)
            .fetch_all(pool)
            .await
        {
            Ok(rows) => rows.into_iter().collect(),
            Err(error) => {
                tracing::warn!(%error, provider, "read music catalog mapping cache");
                HashMap::new()
            }
        }
    }

    async fn cache_track_matches(
        &self,
        provider: &str,
        isrcs: &[String],
        matches: &HashMap<String, String>,
    ) {
        let Some(pool) = &self.cache_pool else {
            return;
        };
        for isrc in isrcs {
            let expiry_days = if matches.contains_key(isrc) { 30 } else { 1 };
            if let Err(error) = sqlx::query("INSERT INTO music_catalog_track_mappings (provider,isrc,target_track_id,expires_at) VALUES ($1,$2,$3,NOW() + ($4 * INTERVAL '1 day')) ON CONFLICT (provider,isrc) DO UPDATE SET target_track_id = EXCLUDED.target_track_id, expires_at = EXCLUDED.expires_at, updated_at = NOW()")
                .bind(provider)
                .bind(isrc)
                .bind(matches.get(isrc))
                .bind(expiry_days)
                .execute(pool)
                .await
            {
                tracing::warn!(%error, provider, %isrc, "write music catalog mapping cache");
            }
        }
    }

    async fn spotify_get<T: for<'de> Deserialize<'de>>(&self, url: String) -> Result<T, String> {
        self.spotify_request(|| {
            self.client
                .get(&url)
                .bearer_auth(&self.spotify_access_token)
        })
        .await?
        .json()
        .await
        .map_err(|error| error.to_string())
    }

    async fn spotify_request<F>(&self, request: F) -> Result<Response, String>
    where
        F: Fn() -> reqwest::RequestBuilder,
    {
        for retry in 0..=SPOTIFY_RATE_LIMIT_RETRIES {
            let response = request()
                .send()
                .await
                .map_err(|error| format!("could not send Spotify request: {error}"))?;
            if response.status() != StatusCode::TOO_MANY_REQUESTS {
                return if response.status().is_success() {
                    Ok(response)
                } else {
                    Err(provider_http_error("Spotify", response).await)
                };
            }

            let delay = retry_after(&response, retry);
            tracing::warn!(
                retry,
                retry_after_seconds = delay.as_secs(),
                "Spotify rate limited music import request"
            );
            if retry == SPOTIFY_RATE_LIMIT_RETRIES || delay > MAX_INLINE_RATE_LIMIT_DELAY {
                return Err(deferred_rate_limit_error("spotify", delay));
            }
            sleep(delay).await;
        }
        unreachable!("rate-limit retry loop always returns")
    }

    async fn tidal_get<T: for<'de> Deserialize<'de>>(&self, url: String) -> Result<T, String> {
        let url = self.tidal_url(&url)?;
        self.tidal_request(|| {
            self.client
                .get(&url)
                .bearer_auth(&self.tidal_access_token)
                .header("accept", TIDAL_MEDIA_TYPE)
        })
        .await?
        .json()
        .await
        .map_err(|error| error.to_string())
    }

    fn tidal_url(&self, url: &str) -> Result<String, String> {
        if reqwest::Url::parse(url).is_ok() {
            return Ok(url.to_string());
        }
        let base = reqwest::Url::parse(&format!("{}/", self.tidal_api_base.trim_end_matches('/')))
            .map_err(|error| format!("invalid TIDAL API base URL: {error}"))?;
        base.join(url.trim_start_matches('/'))
            .map(|url| url.to_string())
            .map_err(|error| format!("invalid TIDAL pagination URL: {error}"))
    }

    async fn tidal_owned_playlists(&self) -> Result<Vec<TidalPlaylist>, String> {
        let mut url = format!("{}/playlists?filter[owners.id]=me", self.tidal_api_base);
        let mut playlists = Vec::new();
        loop {
            let page: TidalPlaylistPage = self.tidal_get(url).await?;
            playlists.extend(page.data.into_iter().map(|playlist| TidalPlaylist {
                id: playlist.id,
                name: playlist.attributes.name,
            }));
            match page.links.and_then(|links| links.next) {
                Some(next) => url = next,
                None => return Ok(playlists),
            }
        }
    }

    async fn tidal_playlist_track_ids(&self, playlist_id: &str) -> Result<Vec<String>, String> {
        let mut url = format!(
            "{}/playlists/{}/relationships/items",
            self.tidal_api_base,
            query_value(playlist_id)
        );
        let mut ids = Vec::new();
        loop {
            let page: TidalRelationshipPage = self.tidal_get(url).await?;
            ids.extend(
                page.data
                    .into_iter()
                    .filter(|item| item.resource_type == "tracks")
                    .map(|item| item.id),
            );
            match page.links.and_then(|links| links.next) {
                Some(next) => url = next,
                None => return Ok(ids),
            }
        }
    }

    async fn tidal_playlist_tracks(&self, playlist_id: &str) -> Result<Vec<SpotifyTrack>, String> {
        let track_ids = self.tidal_playlist_track_ids(playlist_id).await?;
        let mut tracks = Vec::new();
        for ids in track_ids.chunks(TIDAL_ISRC_FILTER_LIMIT) {
            let mut url = reqwest::Url::parse(&format!("{}/tracks", self.tidal_api_base))
                .map_err(|error| error.to_string())?;
            url.query_pairs_mut()
                .extend_pairs(ids.iter().map(|id| ("filter[id]", id)));
            let response: TidalTracksResponse = self.tidal_get(url.to_string()).await?;
            tracks.extend(response.data.into_iter().map(tidal_track));
        }
        Ok(tracks)
    }

    async fn delete_tidal_playlist(
        &self,
        playlist_id: &str,
        idempotency_key: String,
    ) -> Result<(), String> {
        self.tidal_request(|| {
            self.client
                .delete(format!(
                    "{}/playlists/{}",
                    self.tidal_api_base,
                    query_value(playlist_id)
                ))
                .bearer_auth(&self.tidal_access_token)
                .header("accept", TIDAL_MEDIA_TYPE)
                .header("idempotency-key", &idempotency_key)
        })
        .await?;
        Ok(())
    }

    async fn tidal_request<F>(&self, request: F) -> Result<Response, String>
    where
        F: Fn() -> reqwest::RequestBuilder,
    {
        for retry in 0..=TIDAL_RATE_LIMIT_RETRIES {
            wait_for_tidal_request_slot().await;
            let request = request().build().map_err(|error| {
                tracing::warn!(error = ?error, "could not build TIDAL music request");
                format!("could not build TIDAL request: {error}")
            })?;
            let response = self.client.execute(request).await.map_err(|error| {
                tracing::warn!(error = ?error, "could not send TIDAL music request");
                format!("could not send TIDAL request: {error}")
            })?;
            if response.status() != StatusCode::TOO_MANY_REQUESTS {
                return if response.status().is_success() {
                    Ok(response)
                } else {
                    Err(provider_http_error("TIDAL", response).await)
                };
            }

            let delay = retry_after(&response, retry);
            tracing::warn!(
                retry,
                retry_after_seconds = delay.as_secs(),
                "TIDAL rate limited music import request"
            );
            if retry == TIDAL_RATE_LIMIT_RETRIES || delay > MAX_INLINE_RATE_LIMIT_DELAY {
                return Err(deferred_rate_limit_error("tidal", delay));
            }
            defer_tidal_requests(delay).await;
        }
        unreachable!("rate-limit retry loop always returns")
    }

    async fn tidal_add_tracks(
        &self,
        url: String,
        track_ids: &[String],
        idempotency_key: String,
        position_before: Option<&str>,
    ) -> Result<TrackWriteOutcome, String> {
        let data = track_ids
            .iter()
            .map(|id| {
                serde_json::json!({
                    "type": "tracks",
                    "id": id,
                })
            })
            .collect::<Vec<_>>();
        let body = match position_before {
            Some(position_before) => serde_json::json!({
                "data": data,
                "meta": { "positionBefore": position_before },
            }),
            None => serde_json::json!({ "data": data }),
        };
        let response: TidalTrackWriteResponse = self
            .tidal_request(|| {
                self.client
                    .post(&url)
                    .bearer_auth(&self.tidal_access_token)
                    .header("accept", TIDAL_MEDIA_TYPE)
                    .header("content-type", TIDAL_MEDIA_TYPE)
                    .header("idempotency-key", &idempotency_key)
                    .json(&body)
            })
            .await?
            .json()
            .await
            .map_err(|error| error.to_string())?;
        let skipped = response.meta.skipped.len();
        let unmatched = response
            .meta
            .skipped
            .iter()
            .filter(|item| item.reason != TIDAL_ALREADY_PRESENT)
            .count();
        Ok(TrackWriteOutcome {
            imported_items: track_ids.len().saturating_sub(skipped) as i32,
            unmatched_items: unmatched as i32,
        })
    }

    async fn save_tidal_tracks_allowing_existing(
        &self,
        track_ids: &[String],
        idempotency_key: String,
    ) -> Result<TrackWriteOutcome, String> {
        let url = format!(
            "{}/userCollectionTracks/me/relationships/items",
            self.tidal_api_base
        );
        let mut pending = vec![(track_ids.to_vec(), idempotency_key)];
        let mut outcome = TrackWriteOutcome::default();
        while let Some((track_ids, idempotency_key)) = pending.pop() {
            match self
                .tidal_add_tracks(url.clone(), &track_ids, idempotency_key.clone(), None)
                .await
            {
                Ok(result) => {
                    outcome.imported_items += result.imported_items;
                    outcome.unmatched_items += result.unmatched_items;
                }
                Err(error)
                    if error.contains(TIDAL_DUPLICATE_COLLECTION_ITEMS) && track_ids.len() == 1 => {
                }
                Err(error) if error.contains(TIDAL_DUPLICATE_COLLECTION_ITEMS) => {
                    let split_at = track_ids.len() / 2;
                    pending.push((
                        track_ids[split_at..].to_vec(),
                        format!("{idempotency_key}:right"),
                    ));
                    pending.push((
                        track_ids[..split_at].to_vec(),
                        format!("{idempotency_key}:left"),
                    ));
                }
                Err(error) => return Err(error),
            }
        }
        Ok(outcome)
    }
}

fn provider_error_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value.get(key).and_then(|value| match value {
        serde_json::Value::String(value) => Some(value.chars().take(512).collect()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        _ => None,
    })
}

fn provider_error_diagnostic(body: &str) -> (Option<String>, String) {
    let Ok(document) = serde_json::from_str::<serde_json::Value>(body) else {
        let response = body
            .chars()
            .filter(|character| !character.is_control())
            .take(512)
            .collect::<String>();
        return (
            None,
            if response.is_empty() {
                "empty response body".to_string()
            } else {
                format!("unstructured response: {response}")
            },
        );
    };
    let error = document
        .get("errors")
        .and_then(|errors| errors.as_array())
        .and_then(|errors| errors.first())
        .unwrap_or(&document);
    let code = provider_error_field(error, "code");
    let detail = ["detail", "message", "error_description", "error", "title"]
        .iter()
        .find_map(|key| provider_error_field(error, key));
    let status = provider_error_field(error, "status");
    let public_detail = match (&code, &detail) {
        (Some(code), Some(detail)) => Some(format!("{code}: {detail}")),
        (Some(code), None) => Some(code.clone()),
        (None, Some(detail)) => Some(detail.clone()),
        (None, None) => None,
    };
    let diagnostic = [
        code.map(|value| format!("code={value}")),
        status.map(|value| format!("status={value}")),
        detail.map(|value| format!("detail={value}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
    (
        public_detail,
        if diagnostic.is_empty() {
            "JSON response without recognized error fields".to_string()
        } else {
            diagnostic
        },
    )
}

async fn provider_http_error(provider: &str, response: Response) -> String {
    let status = response.status();
    let path = response.url().path().to_string();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("<missing>")
        .to_string();
    let request_id = ["x-request-id", "x-correlation-id", "x-amzn-requestid"]
        .iter()
        .find_map(|header| response.headers().get(*header))
        .and_then(|value| value.to_str().ok())
        .unwrap_or("<missing>")
        .to_string();
    let body = response.text().await.unwrap_or_default();
    let (detail, diagnostic) = provider_error_diagnostic(&body);
    tracing::warn!(
        provider,
        %status,
        endpoint = %path,
        %content_type,
        %request_id,
        provider_error = %diagnostic,
        "music provider API request failed"
    );
    match detail {
        Some(detail) => format!("{provider} returned {status} for {path}: {detail}"),
        None => format!("{provider} returned {status} for {path}"),
    }
}

impl MusicImportProvider for HttpMusicImportProvider {
    async fn tidal_owned_playlists(&self) -> Result<Vec<TidalPlaylist>, String> {
        HttpMusicImportProvider::tidal_owned_playlists(self).await
    }

    async fn tidal_playlist_tracks(&self, playlist_id: &str) -> Result<Vec<SpotifyTrack>, String> {
        HttpMusicImportProvider::tidal_playlist_tracks(self, playlist_id).await
    }

    async fn delete_tidal_playlist(
        &self,
        playlist_id: &str,
        idempotency_key: String,
    ) -> Result<(), String> {
        HttpMusicImportProvider::delete_tidal_playlist(self, playlist_id, idempotency_key).await
    }

    async fn spotify_current_user_id(&self) -> Result<String, String> {
        Ok(self
            .spotify_get::<SpotifyCurrentUser>(format!("{}/v1/me", self.spotify_api_base))
            .await?
            .id)
    }

    async fn spotify_playlists(&self) -> Result<Vec<SpotifyPlaylist>, String> {
        let mut url = format!("{}/v1/me/playlists?limit=50", self.spotify_api_base);
        let mut playlists = Vec::new();
        loop {
            let page: SpotifyPlaylistPage = self.spotify_get(url).await?;
            playlists.extend(page.items.into_iter().map(|playlist| SpotifyPlaylist {
                id: playlist.id,
                name: playlist.name,
                owner_id: playlist.owner.id,
                is_public: playlist.is_public.unwrap_or(false),
            }));
            match page.next {
                Some(next) => url = next,
                None => return Ok(playlists),
            }
        }
    }

    async fn spotify_playlist_tracks(
        &self,
        playlist_id: &str,
    ) -> Result<Vec<SpotifyTrack>, String> {
        let mut url = format!(
            "{}/v1/playlists/{}/items?limit=50",
            self.spotify_api_base,
            query_value(playlist_id)
        );
        let mut tracks = Vec::new();
        loop {
            let page: SpotifyPlaylistTrackPage = self.spotify_get(url).await?;
            tracks.extend(
                page.items
                    .into_iter()
                    .filter_map(spotify_playlist_item)
                    .filter_map(spotify_track),
            );
            match page.next {
                Some(next) => url = next,
                None => return Ok(tracks),
            }
        }
    }

    async fn spotify_saved_tracks(&self) -> Result<Vec<SpotifyTrack>, String> {
        let mut url = format!("{}/v1/me/tracks?limit=50", self.spotify_api_base);
        let mut tracks = Vec::new();
        loop {
            let page: SpotifySavedTrackPage = self.spotify_get(url).await?;
            tracks.extend(
                page.items
                    .into_iter()
                    .filter_map(|item| spotify_track(item.track)),
            );
            match page.next {
                Some(next) => url = next,
                None => return Ok(tracks),
            }
        }
    }

    async fn spotify_tracks_by_isrc(
        &self,
        isrcs: &[String],
    ) -> Result<HashMap<String, String>, String> {
        let cached = self.cached_track_matches("spotify", isrcs).await;
        let mut matches = cached
            .iter()
            .filter_map(|(isrc, track_id)| track_id.as_ref().map(|id| (isrc.clone(), id.clone())))
            .collect::<HashMap<_, _>>();
        let missing = isrcs
            .iter()
            .filter(|isrc| !cached.contains_key(*isrc))
            .cloned()
            .collect::<Vec<_>>();
        for isrc in &missing {
            let result: SpotifySearchResponse = self
                .spotify_get(format!(
                    "{}/v1/search?q={}&type=track&limit=1",
                    self.spotify_api_base,
                    query_value(&format!("isrc:{isrc}"))
                ))
                .await?;
            if let Some(track) = result.tracks.items.into_iter().next()
                && let Some(track_isrc) = track
                    .external_ids
                    .and_then(|ids| ids.isrc)
                    .map(|value| normalize_isrc(&value))
                && track_isrc == *isrc
            {
                matches.insert(isrc.clone(), track.id);
            }
            self.cache_track_matches("spotify", std::slice::from_ref(isrc), &matches)
                .await;
        }
        Ok(matches)
    }

    async fn save_spotify_tracks(&self, track_ids: &[String]) -> Result<(), String> {
        for ids in track_ids.chunks(40) {
            let uris = ids
                .iter()
                .map(|id| format!("spotify:track:{id}"))
                .collect::<Vec<_>>()
                .join(",");
            let url = format!(
                "{}/v1/me/library?uris={}",
                self.spotify_api_base,
                query_value(&uris)
            );
            self.spotify_request(|| {
                self.client
                    .put(&url)
                    .bearer_auth(&self.spotify_access_token)
            })
            .await?;
        }
        Ok(())
    }

    async fn create_spotify_playlist(&self, playlist: &TidalPlaylist) -> Result<String, String> {
        let body = serde_json::json!({ "name": playlist.name, "public": false });
        let response = self
            .spotify_request(|| {
                self.client
                    .post(format!("{}/v1/me/playlists", self.spotify_api_base))
                    .bearer_auth(&self.spotify_access_token)
                    .json(&body)
            })
            .await?;
        response
            .json::<SpotifyPlaylistCreateResponse>()
            .await
            .map(|playlist| playlist.id)
            .map_err(|error| error.to_string())
    }

    async fn add_spotify_playlist_tracks(
        &self,
        playlist_id: &str,
        track_ids: &[String],
    ) -> Result<(), String> {
        let url = format!(
            "{}/v1/playlists/{}/items",
            self.spotify_api_base,
            query_value(playlist_id)
        );
        for ids in track_ids.chunks(100) {
            let body = serde_json::json!({
                "uris": ids.iter().map(|id| format!("spotify:track:{id}")).collect::<Vec<_>>(),
            });
            self.spotify_request(|| {
                self.client
                    .post(&url)
                    .bearer_auth(&self.spotify_access_token)
                    .json(&body)
            })
            .await?;
        }
        Ok(())
    }

    async fn spotify_followed_artists(&self) -> Result<Vec<SpotifyArtist>, String> {
        let mut url = format!(
            "{}/v1/me/following?type=artist&limit=50",
            self.spotify_api_base
        );
        let mut artists = Vec::new();
        loop {
            let page: SpotifyFollowingPage = self.spotify_get(url).await?;
            artists.extend(
                page.artists
                    .items
                    .into_iter()
                    .map(|artist| SpotifyArtist { name: artist.name }),
            );
            match page.artists.next {
                Some(next) => url = next,
                None => return Ok(artists),
            }
        }
    }

    async fn create_tidal_playlist(
        &self,
        playlist: &SpotifyPlaylist,
        visibility: TidalPlaylistVisibility,
        idempotency_key: String,
    ) -> Result<String, String> {
        let body = serde_json::json!({
            "data": {
                "type": "playlists",
                "attributes": {
                    "name": playlist.name,
                    "accessType": visibility.as_api_value(),
                }
            }
        });
        self.tidal_request(|| {
            self.client
                .post(format!("{}/playlists", self.tidal_api_base))
                .bearer_auth(&self.tidal_access_token)
                .header("accept", TIDAL_MEDIA_TYPE)
                .header("content-type", TIDAL_MEDIA_TYPE)
                .header("idempotency-key", &idempotency_key)
                .json(&body)
        })
        .await?
        .json::<TidalPlaylistCreateResponse>()
        .await
        .map(|response| response.data.id)
        .map_err(|error| error.to_string())
    }

    async fn tidal_tracks_by_isrc(
        &self,
        isrcs: &[String],
    ) -> Result<HashMap<String, String>, String> {
        if isrcs.is_empty() {
            return Ok(HashMap::new());
        }
        let cached = self.cached_track_matches("tidal", isrcs).await;
        let mut matches = cached
            .iter()
            .filter_map(|(isrc, track_id)| track_id.as_ref().map(|id| (isrc.clone(), id.clone())))
            .collect::<HashMap<_, _>>();
        let missing = isrcs
            .iter()
            .filter(|isrc| !cached.contains_key(*isrc))
            .cloned()
            .collect::<Vec<_>>();
        for isrcs in missing.chunks(TIDAL_ISRC_FILTER_LIMIT) {
            let mut url = reqwest::Url::parse(&format!("{}/tracks", self.tidal_api_base))
                .map_err(|error| error.to_string())?;
            url.query_pairs_mut()
                .extend_pairs(isrcs.iter().map(|isrc| ("filter[isrc]", isrc)));
            let response: TidalTracksResponse = self.tidal_get(url.to_string()).await?;
            matches.extend(response.data.into_iter().filter_map(|track| {
                track
                    .attributes
                    .isrc
                    .map(|isrc| (normalize_isrc(&isrc), track.id))
            }));
        }
        self.cache_track_matches("tidal", &missing, &matches).await;
        Ok(matches)
    }

    async fn find_tidal_track(&self, track: &SpotifyTrack) -> Result<Option<String>, String> {
        let Some(artist_name) = track.artist_name.as_deref() else {
            return Ok(None);
        };
        let response: TidalTrackSearchResponse = self
            .tidal_get(format!(
                "{}/searchResults/{}?include=tracks",
                self.tidal_api_base,
                query_value(&format!("{} {artist_name}", track.name))
            ))
            .await?;
        let candidate_ids = response
            .included
            .into_iter()
            .flatten()
            .filter(|candidate| {
                candidate
                    .attributes
                    .title
                    .as_deref()
                    .is_some_and(|title| same_track_name(title, &track.name))
            })
            .map(|candidate| candidate.id)
            .collect::<Vec<_>>();
        if candidate_ids.is_empty() {
            return Ok(None);
        }

        let mut url = reqwest::Url::parse(&format!("{}/tracks", self.tidal_api_base))
            .map_err(|error| error.to_string())?;
        url.query_pairs_mut()
            .extend_pairs(candidate_ids.iter().map(|id| ("filter[id]", id)))
            .append_pair("include", "artists");
        let response: TidalTracksWithArtistsResponse = self.tidal_get(url.to_string()).await?;
        let matching_artist_ids = response
            .included
            .into_iter()
            .flatten()
            .filter(|artist| {
                artist.resource_type == "artists"
                    && artist
                        .attributes
                        .as_ref()
                        .is_some_and(|attributes| same_artist_name(&attributes.name, artist_name))
            })
            .map(|artist| artist.id)
            .collect::<HashSet<_>>();

        let matched_id = response
            .data
            .into_iter()
            .find(|candidate| {
                candidate
                    .attributes
                    .title
                    .as_deref()
                    .is_some_and(|title| same_track_name(title, &track.name))
                    && candidate
                        .relationships
                        .artists
                        .data
                        .iter()
                        .any(|artist| matching_artist_ids.contains(&artist.id))
            })
            .map(|candidate| candidate.id);
        if let (Some(isrc), Some(track_id)) = (&track.isrc, &matched_id) {
            self.cache_track_matches(
                "tidal",
                std::slice::from_ref(isrc),
                &HashMap::from([(isrc.clone(), track_id.clone())]),
            )
            .await;
        }
        Ok(matched_id)
    }

    async fn tidal_saved_tracks(&self) -> Result<Vec<TidalSavedTrack>, String> {
        let mut url = format!(
            "{}/userCollectionTracks/me/relationships/items",
            self.tidal_api_base
        );
        let mut ids = Vec::new();
        loop {
            let page: TidalRelationshipPage = self.tidal_get(url).await?;
            ids.extend(
                page.data
                    .into_iter()
                    .filter(|item| item.resource_type == "tracks")
                    .map(|item| item.id),
            );
            match page.links.and_then(|links| links.next) {
                Some(next) => url = next,
                None => break,
            }
        }
        let mut tracks = Vec::new();
        for ids in ids.chunks(TIDAL_ISRC_FILTER_LIMIT) {
            let mut url = reqwest::Url::parse(&format!("{}/tracks", self.tidal_api_base))
                .map_err(|error| error.to_string())?;
            url.query_pairs_mut()
                .extend_pairs(ids.iter().map(|id| ("filter[id]", id)));
            let response: TidalTracksResponse = self.tidal_get(url.to_string()).await?;
            tracks.extend(response.data.into_iter().map(|track| {
                let track = tidal_track(track);
                TidalSavedTrack {
                    isrc: track.isrc,
                    name: track.name,
                    artist_name: track.artist_name,
                    album_name: track.album_name,
                }
            }));
        }
        Ok(tracks)
    }

    async fn add_tidal_playlist_tracks(
        &self,
        playlist_id: &str,
        track_ids: &[String],
        idempotency_key: String,
    ) -> Result<TrackWriteOutcome, String> {
        self.tidal_add_tracks(
            format!(
                "{}/playlists/{}/relationships/items",
                self.tidal_api_base,
                query_value(playlist_id)
            ),
            track_ids,
            idempotency_key,
            None,
        )
        .await
    }

    async fn save_tidal_tracks(
        &self,
        track_ids: &[String],
        idempotency_key: String,
    ) -> Result<TrackWriteOutcome, String> {
        self.save_tidal_tracks_allowing_existing(track_ids, idempotency_key)
            .await
    }

    async fn find_tidal_artist(&self, name: &str) -> Result<Option<String>, String> {
        let response: TidalSearchResponse = self
            .tidal_get(format!(
                "{}/searchResults/{}?include=artists",
                self.tidal_api_base,
                query_value(name)
            ))
            .await?;
        Ok(response
            .included
            .into_iter()
            .flatten()
            .find(|artist| {
                artist.resource_type == "artists"
                    && artist
                        .attributes
                        .as_ref()
                        .is_some_and(|attributes| same_artist_name(&attributes.name, name))
            })
            .map(|artist| artist.id))
    }

    async fn follow_tidal_artists(
        &self,
        artist_ids: &[String],
        idempotency_key: String,
    ) -> Result<FollowOutcome, String> {
        let body = serde_json::json!({
            "data": artist_ids.iter().map(|id| serde_json::json!({
                "type": "artists",
                "id": id,
            })).collect::<Vec<_>>(),
        });
        let response: TidalFollowResponse = self
            .tidal_request(|| {
                self.client
                    .post(format!(
                        "{}/userCollectionArtists/me/relationships/items",
                        self.tidal_api_base
                    ))
                    .bearer_auth(&self.tidal_access_token)
                    .header("accept", TIDAL_MEDIA_TYPE)
                    .header("content-type", TIDAL_MEDIA_TYPE)
                    .header("idempotency-key", &idempotency_key)
                    .json(&body)
            })
            .await?
            .json()
            .await
            .map_err(|error| error.to_string())?;
        let not_found = response
            .meta
            .skipped
            .iter()
            .filter(|item| item.reason == "NOT_FOUND")
            .count() as i32;
        Ok(FollowOutcome {
            imported_items: artist_ids.len() as i32 - not_found,
            unmatched_items: not_found,
        })
    }
}

pub(super) async fn execute_import_with_progress<P, F, Fut, G, PlaylistFut, H, PlaylistHashFut>(
    provider: &P,
    import_id: Uuid,
    options: ImportOptions,
    playlist_mappings: &PlaylistMappings,
    mut report_progress: F,
    mut report_playlist_created: G,
    mut report_playlist_synced: H,
) -> Result<(ImportOutcome, ImportProgress), ImportFailure>
where
    P: MusicImportProvider,
    F: FnMut(ImportProgress) -> Fut,
    Fut: Future<Output = ()>,
    G: FnMut(&SpotifyPlaylist, &str) -> PlaylistFut,
    PlaylistFut: Future<Output = ()>,
    H: FnMut(&SpotifyPlaylist, &str, &[SpotifyTrack]) -> PlaylistHashFut,
    PlaylistHashFut: Future<Output = ()>,
{
    let mut outcome = ImportOutcome::default();
    let mut progress = ImportProgress {
        stage: "reading_spotify",
        activity: "Reading Spotify playlists, liked songs, and followed artists".to_string(),
        ..ImportProgress::default()
    };
    report_progress(progress.clone()).await;
    let spotify_user_id = provider
        .spotify_current_user_id()
        .await
        .map_err(|message| ImportFailure {
            message,
            outcome: outcome.clone(),
            progress: progress.clone(),
        })?;

    let playlists = if options.include_owned_playlists || options.include_saved_playlists {
        provider
            .spotify_playlists()
            .await
            .map_err(|message| ImportFailure {
                message,
                outcome: outcome.clone(),
                progress: progress.clone(),
            })?
            .into_iter()
            .filter(|playlist| {
                (options.include_owned_playlists && playlist.owner_id == spotify_user_id)
                    || (options.include_saved_playlists && playlist.owner_id != spotify_user_id)
            })
            .collect()
    } else {
        Vec::new()
    };
    let mut playlist_tracks = Vec::with_capacity(playlists.len());
    let mut skipped_playlists = 0_i32;
    for playlist in playlists {
        progress.activity = format!("Reading Spotify playlist: {}", playlist.name);
        report_progress(progress.clone()).await;
        match provider.spotify_playlist_tracks(&playlist.id).await {
            Ok(tracks) => playlist_tracks.push((playlist, tracks)),
            Err(message) if is_inaccessible_spotify_playlist_error(&message) => {
                skipped_playlists += 1;
                outcome.unmatched_items += 1;
                outcome.warning = Some(INACCESSIBLE_SPOTIFY_PLAYLIST_WARNING.to_string());
                tracing::warn!(
                    playlist_id = %playlist.id,
                    error = %message,
                    "skipping inaccessible Spotify playlist during music import"
                );
            }
            Err(message) => {
                return Err(ImportFailure {
                    message,
                    outcome: outcome.clone(),
                    progress: progress.clone(),
                });
            }
        }
    }
    let saved_tracks = if options.include_saved_tracks {
        provider
            .spotify_saved_tracks()
            .await
            .map_err(|message| ImportFailure {
                message,
                outcome: outcome.clone(),
                progress: progress.clone(),
            })?
    } else {
        Vec::new()
    };
    let artists = if options.include_followed_artists {
        provider
            .spotify_followed_artists()
            .await
            .map_err(|message| ImportFailure {
                message,
                outcome: outcome.clone(),
                progress: progress.clone(),
            })?
    } else {
        Vec::new()
    };
    outcome.total_items = (skipped_playlists as usize
        + playlist_tracks.len()
        + artists.len()
        + playlist_tracks
            .iter()
            .map(|(_, tracks)| tracks.len())
            .sum::<usize>()
        + saved_tracks.len()) as i32;
    progress.playlist_total = playlist_tracks.len() as i32 + skipped_playlists;
    progress.artist_total = artists.len() as i32;
    progress.playlist_track_total = playlist_tracks
        .iter()
        .map(|(_, tracks)| tracks.len() as i32)
        .sum();
    progress.saved_track_total = saved_tracks.len() as i32;
    progress.stage = if playlist_tracks.is_empty() {
        "matching_artists"
    } else {
        "creating_playlists"
    };
    progress.activity = "Reading your existing TIDAL playlists for reconciliation".to_string();
    report_progress(progress.clone()).await;
    let tidal_playlists =
        provider
            .tidal_owned_playlists()
            .await
            .map_err(|message| ImportFailure {
                message,
                outcome: outcome.clone(),
                progress: progress.clone(),
            })?;
    let mut tidal_playlist_tracks = HashMap::new();
    if options.include_owned_playlists {
        progress.stage = "reconciling_tidal_playlists";
        progress.activity = "Reading existing TIDAL playlists for reconciliation".to_string();
        report_progress(progress.clone()).await;
        for tidal_playlist in &tidal_playlists {
            progress.activity = format!("Reading TIDAL playlist: {}", tidal_playlist.name);
            report_progress(progress.clone()).await;
            let tracks = provider
                .tidal_playlist_tracks(&tidal_playlist.id)
                .await
                .map_err(|message| ImportFailure {
                    message,
                    outcome: outcome.clone(),
                    progress: progress.clone(),
                })?;
            tidal_playlist_tracks.insert(tidal_playlist.id.clone(), tracks);
        }
    }

    let mut consumed_tidal_playlists = HashSet::new();
    for (playlist, tracks) in playlist_tracks {
        let visibility = if playlist.is_public {
            TidalPlaylistVisibility::Public
        } else {
            TidalPlaylistVisibility::Unlisted
        };
        let mapped_tidal_playlist_id = playlist_mappings.tidal_by_spotify.get(&playlist.id);
        let mapped_tidal_playlist = mapped_tidal_playlist_id.and_then(|mapped_id| {
            tidal_playlists
                .iter()
                .find(|candidate| candidate.id == *mapped_id)
        });
        let same_name_tidal_playlists = tidal_playlists
            .iter()
            .filter(|candidate| same_playlist_name(&candidate.name, &playlist.name))
            .collect::<Vec<_>>();
        let canonical_tidal_playlist =
            mapped_tidal_playlist.or_else(|| same_name_tidal_playlists.first().copied());
        let tidal_playlist_id = if let Some(existing) = canonical_tidal_playlist {
            progress.activity = format!("Reusing TIDAL playlist: {}", playlist.name);
            report_progress(progress.clone()).await;
            report_playlist_created(&playlist, &existing.id).await;
            existing.id.clone()
        } else if let Some(existing_id) = mapped_tidal_playlist_id
            && playlist_mappings
                .current_import_spotify
                .contains(&playlist.id)
        {
            progress.activity = format!(
                "Resuming recently created TIDAL playlist: {}",
                playlist.name
            );
            report_progress(progress.clone()).await;
            report_playlist_created(&playlist, existing_id).await;
            existing_id.clone()
        } else {
            progress.activity = format!("Creating TIDAL playlist: {}", playlist.name);
            report_progress(progress.clone()).await;
            let created = provider
                .create_tidal_playlist(
                    &playlist,
                    visibility,
                    idempotency_key(import_id, &format!("playlist:{}", playlist.id)),
                )
                .await
                .map_err(|message| ImportFailure {
                    message,
                    outcome: outcome.clone(),
                    progress: progress.clone(),
                })?;
            report_playlist_created(&playlist, &created).await;
            outcome.imported_items += 1;
            progress.playlists_imported += 1;
            report_progress(progress.clone()).await;
            created
        };

        let tidal_group = if options.include_owned_playlists {
            tidal_playlists
                .iter()
                .filter(|candidate| {
                    candidate.id == tidal_playlist_id
                        || same_playlist_name(&candidate.name, &playlist.name)
                })
                .collect::<Vec<_>>()
        } else {
            canonical_tidal_playlist.into_iter().collect::<Vec<_>>()
        };
        consumed_tidal_playlists.extend(tidal_group.iter().map(|candidate| candidate.id.clone()));
        let all_tidal_tracks = unique_reconciliation_tracks(
            tidal_group
                .iter()
                .flat_map(|candidate| {
                    tidal_playlist_tracks
                        .get(&candidate.id)
                        .into_iter()
                        .flatten()
                })
                .cloned(),
        );

        if options.include_owned_playlists {
            let mut canonical_tidal_track_ids = tidal_playlist_tracks
                .get(&tidal_playlist_id)
                .into_iter()
                .flatten()
                .filter_map(|track| track.source_id.clone())
                .collect::<HashSet<_>>();
            let mut canonical_tidal_isrcs = tidal_playlist_tracks
                .get(&tidal_playlist_id)
                .into_iter()
                .flatten()
                .filter_map(|track| track.isrc.clone())
                .collect::<HashSet<_>>();
            let duplicate_track_ids = all_tidal_tracks
                .iter()
                .filter(|track| {
                    track
                        .isrc
                        .as_ref()
                        .is_none_or(|isrc| canonical_tidal_isrcs.insert(isrc.clone()))
                })
                .filter_map(|track| track.source_id.clone())
                .filter(|track_id| canonical_tidal_track_ids.insert(track_id.clone()))
                .collect::<Vec<_>>();
            for (batch_index, track_ids) in duplicate_track_ids.chunks(50).enumerate() {
                progress.activity =
                    format!("Consolidating TIDAL playlist content: {}", playlist.name);
                report_progress(progress.clone()).await;
                let result = provider
                    .add_tidal_playlist_tracks(
                        &tidal_playlist_id,
                        track_ids,
                        idempotency_key(
                            import_id,
                            &format!("consolidate_playlist:{}:{batch_index}", tidal_playlist_id),
                        ),
                    )
                    .await
                    .map_err(|message| ImportFailure {
                        message,
                        outcome: outcome.clone(),
                        progress: progress.clone(),
                    })?;
                outcome.imported_items += result.imported_items;
                outcome.unmatched_items += result.unmatched_items;
                progress.playlist_tracks_imported += result.imported_items;
                progress.tracks_unmatched += result.unmatched_items;
                if result.unmatched_items > 0 {
                    return Err(ImportFailure {
                        message: format!(
                            "TIDAL skipped {} song(s) while consolidating {}; duplicate playlists were kept",
                            result.unmatched_items, playlist.name
                        ),
                        outcome,
                        progress,
                    });
                }
            }
        }

        progress.stage = "adding_playlist_tracks";
        let tidal_isrcs = all_tidal_tracks
            .iter()
            .filter_map(|track| track.isrc.as_ref())
            .collect::<HashSet<_>>();
        let spotify_only_tracks = unique_reconciliation_tracks(
            tracks
                .iter()
                .filter(|track| {
                    track
                        .isrc
                        .as_ref()
                        .is_none_or(|isrc| !tidal_isrcs.contains(isrc))
                })
                .cloned(),
        );
        progress.activity = format!("Matching Spotify-only songs: {}", playlist.name);
        report_progress(progress.clone()).await;
        let mut desired_tidal_track_ids = Vec::new();
        for tracks in spotify_only_tracks.chunks(50) {
            let (tidal_track_ids, unmatched) =
                match_tidal_tracks(provider, tracks, format!("Playlist: {}", playlist.name))
                    .await
                    .map_err(|message| ImportFailure {
                        message,
                        outcome: outcome.clone(),
                        progress: progress.clone(),
                    })?;
            outcome.unmatched_items += unmatched.len() as i32;
            progress.tracks_unmatched += unmatched.len() as i32;
            outcome.unmatched_tracks.extend(unmatched);
            progress.tracks_matched += tidal_track_ids.len() as i32;
            desired_tidal_track_ids.extend(tidal_track_ids.iter().cloned());
            report_progress(progress.clone()).await;
        }
        if !desired_tidal_track_ids.is_empty() {
            let result = provider
                .add_tidal_playlist_tracks(
                    &tidal_playlist_id,
                    &desired_tidal_track_ids,
                    idempotency_key(import_id, &format!("playlist_tracks_add:{}", playlist.id)),
                )
                .await
                .map_err(|message| ImportFailure {
                    message,
                    outcome: outcome.clone(),
                    progress: progress.clone(),
                })?;
            outcome.imported_items += result.imported_items;
            outcome.unmatched_items += result.unmatched_items;
            progress.playlist_tracks_imported += result.imported_items;
            progress.tracks_unmatched += result.unmatched_items;
        }

        if options.include_owned_playlists {
            let spotify_isrcs = tracks
                .iter()
                .filter_map(|track| track.isrc.as_ref())
                .collect::<HashSet<_>>();
            let tidal_only_tracks = unique_reconciliation_tracks(
                all_tidal_tracks
                    .iter()
                    .filter(|track| {
                        track
                            .isrc
                            .as_ref()
                            .is_none_or(|isrc| !spotify_isrcs.contains(isrc))
                    })
                    .cloned(),
            );
            let (spotify_track_ids, unmatched) = match_spotify_tracks(
                provider,
                &tidal_only_tracks,
                format!("TIDAL Playlist: {}", playlist.name),
            )
            .await
            .map_err(|message| ImportFailure {
                message,
                outcome: outcome.clone(),
                progress: progress.clone(),
            })?;
            outcome.unmatched_items += unmatched.len() as i32;
            progress.tracks_unmatched += unmatched.len() as i32;
            outcome.unmatched_tracks.extend(unmatched);
            progress.tracks_matched += spotify_track_ids.len() as i32;
            if !spotify_track_ids.is_empty() {
                provider
                    .add_spotify_playlist_tracks(&playlist.id, &spotify_track_ids)
                    .await
                    .map_err(|message| ImportFailure {
                        message,
                        outcome: outcome.clone(),
                        progress: progress.clone(),
                    })?;
                outcome.imported_items += spotify_track_ids.len() as i32;
                progress.playlist_tracks_imported += spotify_track_ids.len() as i32;
            }
        }

        for duplicate in tidal_group
            .iter()
            .filter(|candidate| candidate.id != tidal_playlist_id)
        {
            progress.activity = format!("Removing duplicate TIDAL playlist: {}", duplicate.name);
            report_progress(progress.clone()).await;
            provider
                .delete_tidal_playlist(
                    &duplicate.id,
                    idempotency_key(import_id, &format!("delete_duplicate:{}", duplicate.id)),
                )
                .await
                .map_err(|message| ImportFailure {
                    message,
                    outcome: outcome.clone(),
                    progress: progress.clone(),
                })?;
        }
        let reconciled_tracks =
            unique_reconciliation_tracks(tracks.iter().chain(&all_tidal_tracks).cloned());
        progress.activity = format!("Reconciled playlist: {}", playlist.name);
        report_progress(progress.clone()).await;
        report_playlist_synced(&playlist, &tidal_playlist_id, &reconciled_tracks).await;
    }

    if options.include_owned_playlists {
        for tidal_playlist in &tidal_playlists {
            if consumed_tidal_playlists.contains(&tidal_playlist.id) {
                continue;
            }
            let tidal_group = tidal_playlists
                .iter()
                .filter(|candidate| same_playlist_name(&candidate.name, &tidal_playlist.name))
                .collect::<Vec<_>>();
            consumed_tidal_playlists
                .extend(tidal_group.iter().map(|candidate| candidate.id.clone()));
            let all_tidal_tracks = unique_reconciliation_tracks(
                tidal_group
                    .iter()
                    .flat_map(|candidate| {
                        tidal_playlist_tracks
                            .get(&candidate.id)
                            .into_iter()
                            .flatten()
                    })
                    .cloned(),
            );
            let mut canonical_track_ids = tidal_playlist_tracks
                .get(&tidal_playlist.id)
                .into_iter()
                .flatten()
                .filter_map(|track| track.source_id.clone())
                .collect::<HashSet<_>>();
            let mut canonical_isrcs = tidal_playlist_tracks
                .get(&tidal_playlist.id)
                .into_iter()
                .flatten()
                .filter_map(|track| track.isrc.clone())
                .collect::<HashSet<_>>();
            let duplicate_track_ids = all_tidal_tracks
                .iter()
                .filter(|track| {
                    track
                        .isrc
                        .as_ref()
                        .is_none_or(|isrc| canonical_isrcs.insert(isrc.clone()))
                })
                .filter_map(|track| track.source_id.clone())
                .filter(|track_id| canonical_track_ids.insert(track_id.clone()))
                .collect::<Vec<_>>();
            for (batch_index, track_ids) in duplicate_track_ids.chunks(50).enumerate() {
                let result = provider
                    .add_tidal_playlist_tracks(
                        &tidal_playlist.id,
                        track_ids,
                        idempotency_key(
                            import_id,
                            &format!("consolidate_tidal_only:{}:{batch_index}", tidal_playlist.id),
                        ),
                    )
                    .await
                    .map_err(|message| ImportFailure {
                        message,
                        outcome: outcome.clone(),
                        progress: progress.clone(),
                    })?;
                if result.unmatched_items > 0 {
                    return Err(ImportFailure {
                        message: format!(
                            "TIDAL skipped {} song(s) while consolidating {}; duplicate playlists were kept",
                            result.unmatched_items, tidal_playlist.name
                        ),
                        outcome,
                        progress,
                    });
                }
            }
            let spotify_playlist_id = provider
                .create_spotify_playlist(tidal_playlist)
                .await
                .map_err(|message| ImportFailure {
                    message,
                    outcome: outcome.clone(),
                    progress: progress.clone(),
                })?;
            let spotify_playlist = SpotifyPlaylist {
                id: spotify_playlist_id,
                name: tidal_playlist.name.clone(),
                owner_id: spotify_user_id.clone(),
                is_public: false,
            };
            report_playlist_created(&spotify_playlist, &tidal_playlist.id).await;
            let (spotify_track_ids, unmatched) = match_spotify_tracks(
                provider,
                &all_tidal_tracks,
                format!("TIDAL Playlist: {}", tidal_playlist.name),
            )
            .await
            .map_err(|message| ImportFailure {
                message,
                outcome: outcome.clone(),
                progress: progress.clone(),
            })?;
            if !spotify_track_ids.is_empty() {
                provider
                    .add_spotify_playlist_tracks(&spotify_playlist.id, &spotify_track_ids)
                    .await
                    .map_err(|message| ImportFailure {
                        message,
                        outcome: outcome.clone(),
                        progress: progress.clone(),
                    })?;
            }
            outcome.imported_items += 1 + spotify_track_ids.len() as i32;
            outcome.unmatched_items += unmatched.len() as i32;
            progress.playlist_total += 1;
            progress.playlists_imported += 1;
            progress.playlist_track_total += all_tidal_tracks.len() as i32;
            progress.playlist_tracks_imported += spotify_track_ids.len() as i32;
            progress.tracks_matched += spotify_track_ids.len() as i32;
            progress.tracks_unmatched += unmatched.len() as i32;
            outcome.unmatched_tracks.extend(unmatched);
            report_playlist_synced(&spotify_playlist, &tidal_playlist.id, &all_tidal_tracks).await;
            for duplicate in tidal_group
                .iter()
                .filter(|candidate| candidate.id != tidal_playlist.id)
            {
                provider
                    .delete_tidal_playlist(
                        &duplicate.id,
                        idempotency_key(import_id, &format!("delete_duplicate:{}", duplicate.id)),
                    )
                    .await
                    .map_err(|message| ImportFailure {
                        message,
                        outcome: outcome.clone(),
                        progress: progress.clone(),
                    })?;
            }
        }
    }

    progress.stage = "matching_artists";
    progress.activity = "Finding exact TIDAL matches for followed artists".to_string();
    report_progress(progress.clone()).await;
    let mut tidal_artist_ids = HashSet::new();
    let artist_count = artists.len();
    for (index, artist) in artists.iter().enumerate() {
        progress.activity = format!("Checking followed artist: {}", artist.name);
        match provider.find_tidal_artist(&artist.name).await {
            Ok(Some(tidal_artist_id)) => {
                tidal_artist_ids.insert(tidal_artist_id);
            }
            Ok(None) => {
                outcome.unmatched_items += 1;
                progress.artists_unmatched += 1;
            }
            Err(message) => {
                return Err(ImportFailure {
                    message,
                    outcome,
                    progress,
                });
            }
        }
        progress.artists_checked += 1;
        if tidal_artist_ids.len() as i32 > progress.artists_matched {
            progress.artists_matched = tidal_artist_ids.len() as i32;
        }
        if index % 10 == 9 || index + 1 == artist_count {
            report_progress(progress.clone()).await;
        }
    }
    let mut tidal_artist_ids = tidal_artist_ids.into_iter().collect::<Vec<_>>();
    tidal_artist_ids.sort_unstable();
    progress.stage = "following_artists";
    progress.activity = "Following matched artists in TIDAL".to_string();
    report_progress(progress.clone()).await;
    for artist_ids in tidal_artist_ids.chunks(50) {
        let result = provider
            .follow_tidal_artists(
                artist_ids,
                artist_follow_idempotency_key(import_id, artist_ids),
            )
            .await
            .map_err(|message| ImportFailure {
                message,
                outcome: outcome.clone(),
                progress: progress.clone(),
            })?;
        outcome.imported_items += result.imported_items;
        outcome.unmatched_items += result.unmatched_items;
        progress.artists_followed += result.imported_items;
        progress.artists_unmatched += result.unmatched_items;
        report_progress(progress.clone()).await;
    }

    if options.include_saved_tracks {
        progress.stage = "saving_liked_tracks";
        progress.activity = "Reading and matching Liked Songs".to_string();
        report_progress(progress.clone()).await;
        let tidal_saved_tracks =
            provider
                .tidal_saved_tracks()
                .await
                .map_err(|message| ImportFailure {
                    message,
                    outcome: outcome.clone(),
                    progress: progress.clone(),
                })?;
        let spotify_saved_isrcs = saved_tracks
            .iter()
            .filter_map(|track| track.isrc.as_ref())
            .collect::<HashSet<_>>();
        for (batch_index, tracks) in saved_tracks.chunks(50).enumerate() {
            progress.activity = format!("Matching Liked Songs batch {}", batch_index + 1);
            let (tidal_track_ids, unmatched) =
                match_tidal_tracks(provider, tracks, "Liked Songs".to_string())
                    .await
                    .map_err(|message| ImportFailure {
                        message,
                        outcome: outcome.clone(),
                        progress: progress.clone(),
                    })?;
            outcome.unmatched_items += unmatched.len() as i32;
            progress.tracks_unmatched += unmatched.len() as i32;
            outcome.unmatched_tracks.extend(unmatched);
            progress.tracks_matched += tidal_track_ids.len() as i32;
            if !tidal_track_ids.is_empty() {
                progress.activity = format!(
                    "Saving {} matched Liked Songs to TIDAL",
                    tidal_track_ids.len()
                );
                let result = provider
                    .save_tidal_tracks(
                        &tidal_track_ids,
                        idempotency_key(import_id, &format!("saved_tracks:{batch_index}")),
                    )
                    .await
                    .map_err(|message| ImportFailure {
                        message,
                        outcome: outcome.clone(),
                        progress: progress.clone(),
                    })?;
                outcome.imported_items += result.imported_items;
                outcome.unmatched_items += result.unmatched_items;
                progress.saved_tracks_imported += result.imported_items;
                progress.tracks_unmatched += result.unmatched_items;
            }
            report_progress(progress.clone()).await;
        }

        let tidal_only_tracks = tidal_saved_tracks
            .into_iter()
            .filter(|track| {
                track
                    .isrc
                    .as_ref()
                    .is_none_or(|isrc| !spotify_saved_isrcs.contains(isrc))
            })
            .collect::<Vec<_>>();
        progress.saved_track_total += tidal_only_tracks.len() as i32;
        outcome.total_items += tidal_only_tracks.len() as i32;
        let tidal_only_isrcs = tidal_only_tracks
            .iter()
            .filter_map(|track| track.isrc.clone())
            .collect::<Vec<_>>();
        let spotify_matches = provider
            .spotify_tracks_by_isrc(&tidal_only_isrcs)
            .await
            .map_err(|message| ImportFailure {
                message,
                outcome: outcome.clone(),
                progress: progress.clone(),
            })?;
        let mut spotify_track_ids = Vec::new();
        for track in tidal_only_tracks {
            match track.isrc.clone() {
                Some(isrc) => match spotify_matches.get(&isrc) {
                    Some(spotify_track_id) => spotify_track_ids.push(spotify_track_id.clone()),
                    None => {
                        outcome.unmatched_items += 1;
                        progress.tracks_unmatched += 1;
                        outcome.unmatched_tracks.push(unmatched_tidal_saved_track(
                            track,
                            "not_available_in_spotify",
                        ));
                    }
                },
                None => {
                    outcome.unmatched_items += 1;
                    progress.tracks_unmatched += 1;
                    outcome
                        .unmatched_tracks
                        .push(unmatched_tidal_saved_track(track, "missing_isrc"));
                }
            }
        }
        for track_ids in spotify_track_ids.chunks(40) {
            progress.activity = format!(
                "Saving {} TIDAL-only Liked Songs to Spotify",
                track_ids.len()
            );
            provider
                .save_spotify_tracks(track_ids)
                .await
                .map_err(|message| ImportFailure {
                    message,
                    outcome: outcome.clone(),
                    progress: progress.clone(),
                })?;
            outcome.imported_items += track_ids.len() as i32;
            progress.saved_tracks_imported += track_ids.len() as i32;
            progress.tracks_matched += track_ids.len() as i32;
        }
        report_progress(progress.clone()).await;
    }
    progress.activity = "Reconciliation complete".to_string();
    report_progress(progress.clone()).await;
    Ok((outcome, progress))
}

async fn match_spotify_tracks<P>(
    provider: &P,
    tracks: &[SpotifyTrack],
    source_collection: String,
) -> Result<(Vec<String>, Vec<UnmatchedTrack>), String>
where
    P: MusicImportProvider,
{
    let mut seen_isrcs = HashSet::new();
    let isrcs = tracks
        .iter()
        .filter_map(|track| track.isrc.as_ref())
        .filter(|isrc| seen_isrcs.insert((*isrc).clone()))
        .cloned()
        .collect::<Vec<_>>();
    let spotify_tracks = provider.spotify_tracks_by_isrc(&isrcs).await?;
    let spotify_track_ids = tracks
        .iter()
        .filter_map(|track| track.isrc.as_ref())
        .filter_map(|isrc| spotify_tracks.get(isrc))
        .cloned()
        .collect::<Vec<_>>();
    let unmatched = tracks
        .iter()
        .filter(|track| {
            track
                .isrc
                .as_ref()
                .is_none_or(|isrc| !spotify_tracks.contains_key(isrc))
        })
        .cloned()
        .map(|track| UnmatchedTrack {
            reason: if track.isrc.is_some() {
                "not_available_in_spotify"
            } else {
                "missing_isrc"
            },
            source_collection: source_collection.clone(),
            track,
        })
        .collect();
    Ok((spotify_track_ids, unmatched))
}

fn unmatched_tidal_saved_track(track: TidalSavedTrack, reason: &'static str) -> UnmatchedTrack {
    UnmatchedTrack {
        source_collection: "TIDAL Liked Songs".to_string(),
        track: SpotifyTrack {
            source_id: None,
            isrc: track.isrc,
            name: track.name,
            artist_name: track.artist_name,
            album_name: track.album_name,
        },
        reason,
    }
}

fn is_inaccessible_spotify_playlist_error(message: &str) -> bool {
    message.contains("403 Forbidden")
}

async fn match_tidal_tracks<P>(
    provider: &P,
    tracks: &[SpotifyTrack],
    source_collection: String,
) -> Result<(Vec<String>, Vec<UnmatchedTrack>), String>
where
    P: MusicImportProvider,
{
    let mut seen_isrcs = HashSet::new();
    let isrcs = tracks
        .iter()
        .filter_map(|track| track.isrc.as_ref())
        .filter(|isrc| seen_isrcs.insert((*isrc).clone()))
        .cloned()
        .collect::<Vec<_>>();
    let tidal_tracks = provider.tidal_tracks_by_isrc(&isrcs).await?;
    let mut tidal_track_ids = Vec::new();
    let mut unmatched = Vec::new();
    let mut metadata_matches: HashMap<(String, String), Option<String>> = HashMap::new();
    for track in tracks {
        if let Some(track_id) = track.isrc.as_ref().and_then(|isrc| tidal_tracks.get(isrc)) {
            tidal_track_ids.push(track_id.clone());
            continue;
        }

        let metadata_key = track.artist_name.as_ref().map(|artist_name| {
            (
                normalize_track_name(&track.name),
                normalize_artist_name(artist_name),
            )
        });
        let metadata_match = if let Some(key) = metadata_key {
            if let Some(cached) = metadata_matches.get(&key) {
                cached.clone()
            } else {
                let found = provider.find_tidal_track(track).await?;
                metadata_matches.insert(key, found.clone());
                found
            }
        } else {
            None
        };
        if let Some(track_id) = metadata_match {
            tidal_track_ids.push(track_id);
        } else {
            unmatched.push(UnmatchedTrack {
                reason: if track.isrc.is_some() {
                    "not_available_in_tidal"
                } else {
                    "missing_isrc"
                },
                source_collection: source_collection.clone(),
                track: track.clone(),
            });
        }
    }
    Ok((tidal_track_ids, unmatched))
}

fn idempotency_key(import_id: Uuid, purpose: &str) -> String {
    format!("{import_id}-{purpose}")
}

fn artist_follow_idempotency_key(import_id: Uuid, artist_ids: &[String]) -> String {
    let mut hasher = Sha256::new();
    for artist_id in artist_ids {
        hasher.update((artist_id.len() as u64).to_be_bytes());
        hasher.update(artist_id.as_bytes());
    }
    let payload_hash = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    idempotency_key(import_id, &format!("artists:{payload_hash}"))
}

fn same_artist_name(left: &str, right: &str) -> bool {
    normalize_artist_name(left) == normalize_artist_name(right)
}

fn normalize_artist_name(value: &str) -> String {
    value.split_whitespace().map(str::to_lowercase).collect()
}

fn same_track_name(left: &str, right: &str) -> bool {
    normalize_track_name(left) == normalize_track_name(right)
}

fn normalize_track_name(value: &str) -> String {
    value.split_whitespace().map(str::to_lowercase).collect()
}

fn same_playlist_name(left: &str, right: &str) -> bool {
    left.split_whitespace()
        .collect::<String>()
        .eq_ignore_ascii_case(&right.split_whitespace().collect::<String>())
}

#[derive(Deserialize)]
struct SpotifyCurrentUser {
    id: String,
}

#[derive(Deserialize)]
struct SpotifyPlaylistPage {
    items: Vec<SpotifyPlaylistWire>,
    next: Option<String>,
}

#[derive(Deserialize)]
struct SpotifyPlaylistWire {
    id: String,
    name: String,
    #[serde(rename = "public")]
    is_public: Option<bool>,
    owner: SpotifyOwner,
}

#[derive(Deserialize)]
struct SpotifyPlaylistCreateResponse {
    id: String,
}

#[derive(Deserialize)]
struct SpotifyOwner {
    id: String,
}

#[derive(Deserialize)]
struct SpotifyFollowingPage {
    artists: SpotifyArtistPage,
}

#[derive(Deserialize)]
struct SpotifyPlaylistTrackPage {
    items: Vec<SpotifyPlaylistTrackItem>,
    next: Option<String>,
}

#[derive(Deserialize)]
struct SpotifyPlaylistTrackItem {
    #[serde(default)]
    item: Option<serde_json::Value>,
    #[serde(default)]
    track: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct SpotifySavedTrackPage {
    items: Vec<SpotifySavedTrackItem>,
    next: Option<String>,
}

#[derive(Deserialize)]
struct SpotifySavedTrackItem {
    track: SpotifyTrackWire,
}

#[derive(Deserialize)]
struct SpotifyTrackWire {
    id: Option<String>,
    #[serde(rename = "type")]
    resource_type: Option<String>,
    #[serde(default)]
    is_local: bool,
    external_ids: Option<SpotifyExternalIds>,
    name: String,
    #[serde(default)]
    artists: Vec<SpotifyArtistWire>,
    album: Option<SpotifyAlbumWire>,
}

#[derive(Deserialize)]
struct SpotifyAlbumWire {
    name: String,
}

#[derive(Deserialize)]
struct SpotifyExternalIds {
    isrc: Option<String>,
}

#[derive(Deserialize)]
struct SpotifySearchResponse {
    tracks: SpotifySearchTracks,
}
#[derive(Deserialize)]
struct SpotifySearchTracks {
    items: Vec<SpotifySearchTrack>,
}
#[derive(Deserialize)]
struct SpotifySearchTrack {
    id: String,
    external_ids: Option<SpotifyExternalIds>,
}

#[derive(Deserialize)]
struct SpotifyArtistPage {
    items: Vec<SpotifyArtistWire>,
    next: Option<String>,
}

#[derive(Deserialize)]
struct SpotifyArtistWire {
    name: String,
}

fn spotify_track(track: SpotifyTrackWire) -> Option<SpotifyTrack> {
    (track.resource_type.as_deref().unwrap_or("track") == "track" && !track.is_local).then_some(
        SpotifyTrack {
            source_id: track.id,
            isrc: track
                .external_ids
                .and_then(|ids| ids.isrc)
                .map(|isrc| normalize_isrc(&isrc)),
            name: track.name,
            artist_name: track.artists.first().map(|artist| artist.name.clone()),
            album_name: track.album.map(|album| album.name),
        },
    )
}

fn spotify_playlist_item(item: SpotifyPlaylistTrackItem) -> Option<SpotifyTrackWire> {
    item.item
        .or(item.track)
        .and_then(|value| serde_json::from_value(value).ok())
}

fn tidal_track(track: TidalTrackResource) -> SpotifyTrack {
    let TidalTrackResource { id, attributes } = track;
    SpotifyTrack {
        source_id: Some(id),
        isrc: attributes.isrc.map(|value| normalize_isrc(&value)),
        name: attributes
            .title
            .unwrap_or_else(|| "TIDAL track".to_string()),
        artist_name: attributes.artist_name,
        album_name: attributes.album_name,
    }
}

fn normalize_isrc(isrc: &str) -> String {
    isrc.trim().to_ascii_uppercase()
}

#[derive(Deserialize)]
struct TidalPlaylistCreateResponse {
    data: TidalPlaylistResource,
}

#[derive(Deserialize)]
struct TidalPlaylistResource {
    id: String,
}

#[derive(Deserialize)]
struct TidalPlaylistPage {
    data: Vec<TidalPlaylistWithAttributes>,
    links: Option<TidalPageLinks>,
}
#[derive(Deserialize)]
struct TidalPlaylistWithAttributes {
    id: String,
    attributes: TidalPlaylistAttributes,
}
#[derive(Deserialize)]
struct TidalPlaylistAttributes {
    name: String,
}
#[derive(Deserialize)]
struct TidalRelationshipPage {
    data: Vec<TidalRelationshipItem>,
    links: Option<TidalPageLinks>,
}
#[derive(Deserialize)]
struct TidalRelationshipItem {
    id: String,
    #[serde(rename = "type")]
    resource_type: String,
}
#[derive(Deserialize)]
struct TidalPageLinks {
    next: Option<String>,
}

#[derive(Deserialize)]
struct TidalTracksResponse {
    data: Vec<TidalTrackResource>,
}

#[derive(Deserialize)]
struct TidalTrackSearchResponse {
    included: Option<Vec<TidalTrackResource>>,
}

#[derive(Deserialize)]
struct TidalTrackResource {
    id: String,
    attributes: TidalTrackAttributes,
}

#[derive(Deserialize)]
struct TidalTracksWithArtistsResponse {
    data: Vec<TidalTrackWithArtists>,
    included: Option<Vec<TidalArtistResource>>,
}

#[derive(Deserialize)]
struct TidalTrackWithArtists {
    id: String,
    attributes: TidalTrackAttributes,
    #[serde(default)]
    relationships: TidalTrackRelationships,
}

#[derive(Default, Deserialize)]
struct TidalTrackRelationships {
    #[serde(default)]
    artists: TidalRelationshipData,
}

#[derive(Default, Deserialize)]
struct TidalRelationshipData {
    #[serde(default)]
    data: Vec<TidalRelationshipItem>,
}

#[derive(Deserialize)]
struct TidalTrackAttributes {
    isrc: Option<String>,
    #[serde(default, alias = "name")]
    title: Option<String>,
    #[serde(default, rename = "artistName", alias = "artist_name")]
    artist_name: Option<String>,
    #[serde(default, rename = "albumName", alias = "album_name")]
    album_name: Option<String>,
}

#[derive(Deserialize)]
struct TidalSearchResponse {
    included: Option<Vec<TidalArtistResource>>,
}

#[derive(Deserialize)]
struct TidalArtistResource {
    id: String,
    #[serde(rename = "type")]
    resource_type: String,
    attributes: Option<TidalArtistAttributes>,
}

#[derive(Deserialize)]
struct TidalArtistAttributes {
    name: String,
}

#[derive(Deserialize, Default)]
struct TidalFollowResponse {
    #[serde(default)]
    meta: TidalFollowMeta,
}

#[derive(Deserialize, Default)]
struct TidalFollowMeta {
    #[serde(default)]
    skipped: Vec<TidalSkippedArtist>,
}

#[derive(Deserialize)]
struct TidalSkippedArtist {
    reason: String,
}

#[derive(Deserialize, Default)]
struct TidalTrackWriteResponse {
    #[serde(default)]
    meta: TidalTrackWriteMeta,
}

#[derive(Deserialize, Default)]
struct TidalTrackWriteMeta {
    #[serde(default)]
    skipped: Vec<TidalSkippedTrack>,
}

#[derive(Deserialize)]
struct TidalSkippedTrack {
    reason: String,
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;
    use axum::{
        Router,
        http::{HeaderMap, StatusCode, Uri, header},
        response::IntoResponse,
        routing::{get, post},
    };

    #[derive(Default)]
    struct FakeProvider {
        playlists: Vec<SpotifyPlaylist>,
        playlist_tracks: HashMap<String, Vec<SpotifyTrack>>,
        playlist_track_errors: HashMap<String, String>,
        tidal_playlists: Vec<TidalPlaylist>,
        tidal_playlist_tracks: HashMap<String, Vec<SpotifyTrack>>,
        saved_tracks: Vec<SpotifyTrack>,
        tidal_saved_tracks: Vec<TidalSavedTrack>,
        tidal_tracks: HashMap<String, String>,
        spotify_tracks: HashMap<String, String>,
        artists: Vec<SpotifyArtist>,
        matched_artist: Option<String>,
        created_playlists: Mutex<Vec<(String, TidalPlaylistVisibility)>>,
        added_playlist_tracks: Mutex<Vec<(String, Vec<String>)>>,
        saved_tidal_tracks: Mutex<Vec<String>>,
        saved_spotify_tracks: Mutex<Vec<String>>,
        created_spotify_playlists: Mutex<Vec<String>>,
        added_spotify_playlist_tracks: Mutex<Vec<(String, Vec<String>)>>,
        followed_artists: Mutex<Vec<(Vec<String>, String)>>,
    }

    impl MusicImportProvider for FakeProvider {
        async fn spotify_current_user_id(&self) -> Result<String, String> {
            Ok("spotify-user".to_string())
        }

        async fn spotify_playlists(&self) -> Result<Vec<SpotifyPlaylist>, String> {
            Ok(self.playlists.clone())
        }

        async fn spotify_playlist_tracks(
            &self,
            playlist_id: &str,
        ) -> Result<Vec<SpotifyTrack>, String> {
            if let Some(error) = self.playlist_track_errors.get(playlist_id) {
                return Err(error.clone());
            }
            Ok(self
                .playlist_tracks
                .get(playlist_id)
                .cloned()
                .unwrap_or_default())
        }

        async fn spotify_saved_tracks(&self) -> Result<Vec<SpotifyTrack>, String> {
            Ok(self.saved_tracks.clone())
        }
        async fn spotify_tracks_by_isrc(
            &self,
            isrcs: &[String],
        ) -> Result<HashMap<String, String>, String> {
            Ok(isrcs
                .iter()
                .filter_map(|isrc| {
                    self.spotify_tracks
                        .get(isrc)
                        .map(|id| (isrc.clone(), id.clone()))
                })
                .collect())
        }
        async fn save_spotify_tracks(&self, track_ids: &[String]) -> Result<(), String> {
            self.saved_spotify_tracks
                .lock()
                .unwrap()
                .extend_from_slice(track_ids);
            Ok(())
        }
        async fn create_spotify_playlist(
            &self,
            playlist: &TidalPlaylist,
        ) -> Result<String, String> {
            self.created_spotify_playlists
                .lock()
                .unwrap()
                .push(playlist.name.clone());
            Ok(format!("spotify-{}", playlist.id))
        }
        async fn add_spotify_playlist_tracks(
            &self,
            playlist_id: &str,
            track_ids: &[String],
        ) -> Result<(), String> {
            self.added_spotify_playlist_tracks
                .lock()
                .unwrap()
                .push((playlist_id.to_string(), track_ids.to_vec()));
            Ok(())
        }

        async fn spotify_followed_artists(&self) -> Result<Vec<SpotifyArtist>, String> {
            Ok(self.artists.clone())
        }

        async fn create_tidal_playlist(
            &self,
            playlist: &SpotifyPlaylist,
            visibility: TidalPlaylistVisibility,
            _idempotency_key: String,
        ) -> Result<String, String> {
            self.created_playlists
                .lock()
                .unwrap()
                .push((playlist.id.clone(), visibility));
            Ok(format!("tidal-{}", playlist.id))
        }

        async fn tidal_owned_playlists(&self) -> Result<Vec<TidalPlaylist>, String> {
            Ok(self.tidal_playlists.clone())
        }
        async fn tidal_playlist_tracks(
            &self,
            playlist_id: &str,
        ) -> Result<Vec<SpotifyTrack>, String> {
            Ok(self
                .tidal_playlist_tracks
                .get(playlist_id)
                .cloned()
                .unwrap_or_default())
        }
        async fn delete_tidal_playlist(
            &self,
            playlist_id: &str,
            _idempotency_key: String,
        ) -> Result<(), String> {
            self.created_playlists.lock().unwrap().push((
                format!("deleted:{playlist_id}"),
                TidalPlaylistVisibility::Public,
            ));
            Ok(())
        }

        async fn tidal_tracks_by_isrc(
            &self,
            isrcs: &[String],
        ) -> Result<HashMap<String, String>, String> {
            Ok(isrcs
                .iter()
                .filter_map(|isrc| {
                    self.tidal_tracks
                        .get(isrc)
                        .map(|track_id| (isrc.clone(), track_id.clone()))
                })
                .collect())
        }
        async fn find_tidal_track(&self, track: &SpotifyTrack) -> Result<Option<String>, String> {
            Ok((same_track_name(&track.name, "First Day of My Life")
                && track
                    .artist_name
                    .as_deref()
                    .is_some_and(|artist| same_artist_name(artist, "Bright Eyes")))
            .then(|| "tidal-metadata-match".to_string()))
        }
        async fn tidal_saved_tracks(&self) -> Result<Vec<TidalSavedTrack>, String> {
            Ok(self.tidal_saved_tracks.clone())
        }

        async fn add_tidal_playlist_tracks(
            &self,
            playlist_id: &str,
            track_ids: &[String],
            _idempotency_key: String,
        ) -> Result<TrackWriteOutcome, String> {
            self.added_playlist_tracks
                .lock()
                .unwrap()
                .push((playlist_id.to_string(), track_ids.to_vec()));
            let unmatched_items = track_ids
                .iter()
                .filter(|track_id| track_id.as_str() == "tidal-skipped-track")
                .count() as i32;
            Ok(TrackWriteOutcome {
                imported_items: track_ids.len() as i32 - unmatched_items,
                unmatched_items,
            })
        }

        async fn save_tidal_tracks(
            &self,
            track_ids: &[String],
            _idempotency_key: String,
        ) -> Result<TrackWriteOutcome, String> {
            self.saved_tidal_tracks
                .lock()
                .unwrap()
                .extend_from_slice(track_ids);
            Ok(TrackWriteOutcome {
                imported_items: track_ids.len() as i32,
                unmatched_items: 0,
            })
        }

        async fn find_tidal_artist(&self, name: &str) -> Result<Option<String>, String> {
            Ok(if self.matched_artist.as_deref() == Some("match-by-name") {
                Some(name.to_string())
            } else {
                self.matched_artist.clone()
            })
        }

        async fn follow_tidal_artists(
            &self,
            artist_ids: &[String],
            idempotency_key: String,
        ) -> Result<FollowOutcome, String> {
            self.followed_artists
                .lock()
                .unwrap()
                .push((artist_ids.to_vec(), idempotency_key));
            Ok(FollowOutcome {
                imported_items: artist_ids.len() as i32,
                unmatched_items: 0,
            })
        }
    }

    fn playlist(id: &str, owner_id: &str, is_public: bool) -> SpotifyPlaylist {
        SpotifyPlaylist {
            id: id.to_string(),
            name: id.to_string(),
            owner_id: owner_id.to_string(),
            is_public,
        }
    }

    fn track(isrc: Option<&str>) -> SpotifyTrack {
        SpotifyTrack {
            source_id: None,
            isrc: isrc.map(str::to_string),
            name: "Track".to_string(),
            artist_name: Some("Artist".to_string()),
            album_name: Some("Album".to_string()),
        }
    }

    fn sourced_track(source_id: &str, isrc: &str) -> SpotifyTrack {
        SpotifyTrack {
            source_id: Some(source_id.to_string()),
            ..track(Some(isrc))
        }
    }

    #[tokio::test]
    async fn falls_back_to_exact_title_and_artist_when_tidal_uses_a_different_isrc() {
        let provider = FakeProvider::default();
        let tracks = vec![SpotifyTrack {
            source_id: Some("spotify-track".to_string()),
            isrc: Some("US-SPOTIFY-ISRC".to_string()),
            name: "First Day of My Life".to_string(),
            artist_name: Some("Bright Eyes".to_string()),
            album_name: Some("I'm Wide Awake, It's Morning".to_string()),
        }];

        let (matches, unmatched) =
            match_tidal_tracks(&provider, &tracks, "Liked Songs".to_string())
                .await
                .unwrap();

        assert_eq!(matches, vec!["tidal-metadata-match".to_string()]);
        assert!(unmatched.is_empty());
    }

    #[test]
    fn keeps_tidal_track_metadata_for_unmatched_audits() {
        let resource = serde_json::from_str::<TidalTrackResource>(
            r#"{"id":"tidal-track","attributes":{"isrc":"us-aaa-01","title":"Actual TIDAL title","artistName":"Actual artist","albumName":"Actual album"}}"#,
        )
        .unwrap();

        assert_eq!(
            tidal_track(resource),
            SpotifyTrack {
                source_id: Some("tidal-track".to_string()),
                isrc: Some("US-AAA-01".to_string()),
                name: "Actual TIDAL title".to_string(),
                artist_name: Some("Actual artist".to_string()),
                album_name: Some("Actual album".to_string()),
            }
        );
    }

    #[test]
    fn playlist_hash_uses_track_order_and_exact_isrcs() {
        let first = vec![track(Some("US-AAA-01")), track(Some("US-BBB-02"))];
        let reordered = vec![track(Some("US-BBB-02")), track(Some("US-AAA-01"))];

        assert_eq!(playlist_content_hash(&first), playlist_content_hash(&first));
        assert_ne!(
            playlist_content_hash(&first),
            playlist_content_hash(&reordered)
        );
    }

    #[tokio::test]
    async fn imports_owned_playlists_with_matching_visibility_and_follows_exact_artist_matches() {
        let provider = FakeProvider {
            playlists: vec![
                playlist("public-owned", "spotify-user", true),
                playlist("private-owned", "spotify-user", false),
                playlist("saved", "other-user", true),
            ],
            playlist_tracks: HashMap::new(),
            playlist_track_errors: HashMap::new(),
            tidal_playlists: Vec::new(),
            tidal_playlist_tracks: HashMap::new(),
            saved_tracks: Vec::new(),
            tidal_saved_tracks: Vec::new(),
            tidal_tracks: HashMap::new(),
            spotify_tracks: HashMap::new(),
            artists: vec![SpotifyArtist {
                name: "The Artist".to_string(),
            }],
            matched_artist: Some("tidal-artist".to_string()),
            created_playlists: Mutex::new(Vec::new()),
            added_playlist_tracks: Mutex::new(Vec::new()),
            saved_tidal_tracks: Mutex::new(Vec::new()),
            saved_spotify_tracks: Mutex::new(Vec::new()),
            created_spotify_playlists: Mutex::new(Vec::new()),
            added_spotify_playlist_tracks: Mutex::new(Vec::new()),
            followed_artists: Mutex::new(Vec::new()),
        };

        let outcome = execute_import_with_progress(
            &provider,
            Uuid::nil(),
            ImportOptions {
                include_owned_playlists: true,
                include_saved_playlists: false,
                include_followed_artists: true,
                include_saved_tracks: false,
            },
            &PlaylistMappings::default(),
            |_| async {},
            |_, _| async {},
            |_, _, _| async {},
        )
        .await
        .unwrap()
        .0;

        assert_eq!(outcome.total_items, 3);
        assert_eq!(outcome.imported_items, 3);
        assert_eq!(outcome.unmatched_items, 0);
        assert_eq!(
            *provider.created_playlists.lock().unwrap(),
            vec![
                ("public-owned".to_string(), TidalPlaylistVisibility::Public),
                (
                    "private-owned".to_string(),
                    TidalPlaylistVisibility::Unlisted
                ),
            ]
        );
        assert_eq!(
            provider.followed_artists.lock().unwrap()[0].0,
            vec!["tidal-artist".to_string()]
        );
    }

    #[tokio::test]
    async fn consolidates_same_name_tidal_playlists_despite_a_stale_mapping() {
        let provider = FakeProvider {
            playlists: vec![playlist("owned", "spotify-user", true)],
            playlist_tracks: HashMap::from([(
                "owned".to_string(),
                vec![track(Some("US-SPOTIFY-01"))],
            )]),
            playlist_track_errors: HashMap::new(),
            tidal_playlists: vec![
                TidalPlaylist {
                    id: "canonical".to_string(),
                    name: "owned".to_string(),
                },
                TidalPlaylist {
                    id: "duplicate".to_string(),
                    name: "owned".to_string(),
                },
            ],
            tidal_playlist_tracks: HashMap::from([
                (
                    "canonical".to_string(),
                    vec![sourced_track("tidal-canonical-track", "US-CANONICAL-02")],
                ),
                (
                    "duplicate".to_string(),
                    vec![sourced_track("tidal-duplicate-track", "US-DUPLICATE-03")],
                ),
            ]),
            saved_tracks: Vec::new(),
            tidal_saved_tracks: Vec::new(),
            tidal_tracks: HashMap::from([(
                "US-SPOTIFY-01".to_string(),
                "tidal-spotify-track".to_string(),
            )]),
            spotify_tracks: HashMap::from([
                (
                    "US-CANONICAL-02".to_string(),
                    "spotify-canonical-track".to_string(),
                ),
                (
                    "US-DUPLICATE-03".to_string(),
                    "spotify-duplicate-track".to_string(),
                ),
            ]),
            artists: Vec::new(),
            matched_artist: None,
            created_playlists: Mutex::new(Vec::new()),
            added_playlist_tracks: Mutex::new(Vec::new()),
            saved_tidal_tracks: Mutex::new(Vec::new()),
            saved_spotify_tracks: Mutex::new(Vec::new()),
            created_spotify_playlists: Mutex::new(Vec::new()),
            added_spotify_playlist_tracks: Mutex::new(Vec::new()),
            followed_artists: Mutex::new(Vec::new()),
        };

        let activities = Arc::new(Mutex::new(Vec::new()));
        execute_import_with_progress(
            &provider,
            Uuid::nil(),
            ImportOptions {
                include_owned_playlists: true,
                include_saved_playlists: false,
                include_followed_artists: false,
                include_saved_tracks: false,
            },
            &PlaylistMappings {
                tidal_by_spotify: HashMap::from([(
                    "owned".to_string(),
                    "deleted-tidal-playlist".to_string(),
                )]),
                ..Default::default()
            },
            {
                let activities = activities.clone();
                move |progress| {
                    let activities = activities.clone();
                    async move { activities.lock().unwrap().push(progress.activity) }
                }
            },
            |_, _| async {},
            |_, _, _| async {},
        )
        .await
        .unwrap();

        assert_eq!(
            *provider.created_playlists.lock().unwrap(),
            vec![(
                "deleted:duplicate".to_string(),
                TidalPlaylistVisibility::Public,
            )]
        );
        assert!(
            activities
                .lock()
                .unwrap()
                .iter()
                .any(|activity| { activity == "Removing duplicate TIDAL playlist: owned" })
        );
        assert_eq!(
            *provider.added_playlist_tracks.lock().unwrap(),
            vec![
                (
                    "canonical".to_string(),
                    vec!["tidal-duplicate-track".to_string()]
                ),
                (
                    "canonical".to_string(),
                    vec!["tidal-spotify-track".to_string()]
                ),
            ]
        );
        assert_eq!(
            *provider.added_spotify_playlist_tracks.lock().unwrap(),
            vec![(
                "owned".to_string(),
                vec![
                    "spotify-canonical-track".to_string(),
                    "spotify-duplicate-track".to_string(),
                ]
            )]
        );
    }

    #[tokio::test]
    async fn keeps_duplicate_tidal_playlists_when_content_cannot_be_consolidated() {
        let provider = FakeProvider {
            playlists: vec![playlist("owned", "spotify-user", true)],
            playlist_tracks: HashMap::from([("owned".to_string(), Vec::new())]),
            playlist_track_errors: HashMap::new(),
            tidal_playlists: vec![
                TidalPlaylist {
                    id: "canonical".to_string(),
                    name: "owned".to_string(),
                },
                TidalPlaylist {
                    id: "duplicate".to_string(),
                    name: "owned".to_string(),
                },
            ],
            tidal_playlist_tracks: HashMap::from([(
                "duplicate".to_string(),
                vec![sourced_track("tidal-skipped-track", "US-SKIPPED-01")],
            )]),
            saved_tracks: Vec::new(),
            tidal_saved_tracks: Vec::new(),
            tidal_tracks: HashMap::new(),
            spotify_tracks: HashMap::new(),
            artists: Vec::new(),
            matched_artist: None,
            created_playlists: Mutex::new(Vec::new()),
            added_playlist_tracks: Mutex::new(Vec::new()),
            saved_tidal_tracks: Mutex::new(Vec::new()),
            saved_spotify_tracks: Mutex::new(Vec::new()),
            created_spotify_playlists: Mutex::new(Vec::new()),
            added_spotify_playlist_tracks: Mutex::new(Vec::new()),
            followed_artists: Mutex::new(Vec::new()),
        };

        let failure = execute_import_with_progress(
            &provider,
            Uuid::nil(),
            ImportOptions {
                include_owned_playlists: true,
                include_saved_playlists: false,
                include_followed_artists: false,
                include_saved_tracks: false,
            },
            &PlaylistMappings::default(),
            |_| async {},
            |_, _| async {},
            |_, _, _| async {},
        )
        .await
        .unwrap_err();

        assert!(failure.message.contains("duplicate playlists were kept"));
        assert!(
            provider
                .created_playlists
                .lock()
                .unwrap()
                .iter()
                .all(|(playlist_id, _)| playlist_id != "deleted:duplicate")
        );
    }

    #[tokio::test]
    async fn includes_saved_playlists_only_when_requested() {
        let provider = FakeProvider {
            playlists: vec![
                playlist("owned", "spotify-user", true),
                playlist("saved", "other-user", false),
            ],
            playlist_tracks: HashMap::new(),
            playlist_track_errors: HashMap::new(),
            tidal_playlists: Vec::new(),
            tidal_playlist_tracks: HashMap::new(),
            saved_tracks: Vec::new(),
            tidal_saved_tracks: Vec::new(),
            tidal_tracks: HashMap::new(),
            spotify_tracks: HashMap::new(),
            artists: Vec::new(),
            matched_artist: None,
            created_playlists: Mutex::new(Vec::new()),
            added_playlist_tracks: Mutex::new(Vec::new()),
            saved_tidal_tracks: Mutex::new(Vec::new()),
            saved_spotify_tracks: Mutex::new(Vec::new()),
            created_spotify_playlists: Mutex::new(Vec::new()),
            added_spotify_playlist_tracks: Mutex::new(Vec::new()),
            followed_artists: Mutex::new(Vec::new()),
        };

        let outcome = execute_import_with_progress(
            &provider,
            Uuid::nil(),
            ImportOptions {
                include_owned_playlists: false,
                include_saved_playlists: true,
                include_followed_artists: false,
                include_saved_tracks: false,
            },
            &PlaylistMappings::default(),
            |_| async {},
            |_, _| async {},
            |_, _, _| async {},
        )
        .await
        .unwrap()
        .0;

        assert_eq!(outcome.total_items, 1);
        assert_eq!(outcome.imported_items, 1);
        assert_eq!(
            *provider.created_playlists.lock().unwrap(),
            vec![("saved".to_string(), TidalPlaylistVisibility::Unlisted)]
        );
    }

    #[tokio::test]
    async fn restart_reuses_persisted_tidal_playlist_without_creating_a_duplicate() {
        let provider = FakeProvider {
            playlists: vec![playlist("owned", "spotify-user", true)],
            playlist_tracks: HashMap::from([("owned".to_string(), vec![track(Some("US-AAA-01"))])]),
            playlist_track_errors: HashMap::new(),
            tidal_playlists: Vec::new(),
            tidal_playlist_tracks: HashMap::new(),
            saved_tracks: Vec::new(),
            tidal_saved_tracks: Vec::new(),
            tidal_tracks: HashMap::from([("US-AAA-01".to_string(), "tidal-track".to_string())]),
            spotify_tracks: HashMap::new(),
            artists: Vec::new(),
            matched_artist: None,
            created_playlists: Mutex::new(Vec::new()),
            added_playlist_tracks: Mutex::new(Vec::new()),
            saved_tidal_tracks: Mutex::new(Vec::new()),
            saved_spotify_tracks: Mutex::new(Vec::new()),
            created_spotify_playlists: Mutex::new(Vec::new()),
            added_spotify_playlist_tracks: Mutex::new(Vec::new()),
            followed_artists: Mutex::new(Vec::new()),
        };

        let (_, progress) = execute_import_with_progress(
            &provider,
            Uuid::nil(),
            ImportOptions {
                include_owned_playlists: true,
                include_saved_playlists: false,
                include_followed_artists: false,
                include_saved_tracks: false,
            },
            &PlaylistMappings {
                tidal_by_spotify: HashMap::from([(
                    "owned".to_string(),
                    "existing-tidal-playlist".to_string(),
                )]),
                current_import_spotify: HashSet::from(["owned".to_string()]),
            },
            |_| async {},
            |_, _| async {},
            |_, _, _| async {},
        )
        .await
        .unwrap();

        assert!(provider.created_playlists.lock().unwrap().is_empty());
        assert_eq!(progress.playlists_imported, 0);
    }

    #[tokio::test]
    async fn imports_exact_isrc_playlist_tracks_and_saved_tracks() {
        let provider = FakeProvider {
            playlists: vec![playlist("owned", "spotify-user", true)],
            playlist_tracks: HashMap::from([(
                "owned".to_string(),
                vec![
                    track(Some("US-AAA-01")),
                    track(Some("US-MISSING-02")),
                    track(None),
                ],
            )]),
            playlist_track_errors: HashMap::new(),
            tidal_playlists: Vec::new(),
            tidal_playlist_tracks: HashMap::new(),
            saved_tracks: vec![track(Some("US-BBB-03"))],
            tidal_saved_tracks: Vec::new(),
            tidal_tracks: HashMap::from([
                ("US-AAA-01".to_string(), "tidal-a".to_string()),
                ("US-BBB-03".to_string(), "tidal-b".to_string()),
            ]),
            spotify_tracks: HashMap::new(),
            artists: Vec::new(),
            matched_artist: None,
            created_playlists: Mutex::new(Vec::new()),
            added_playlist_tracks: Mutex::new(Vec::new()),
            saved_tidal_tracks: Mutex::new(Vec::new()),
            saved_spotify_tracks: Mutex::new(Vec::new()),
            created_spotify_playlists: Mutex::new(Vec::new()),
            added_spotify_playlist_tracks: Mutex::new(Vec::new()),
            followed_artists: Mutex::new(Vec::new()),
        };

        let (outcome, progress) = execute_import_with_progress(
            &provider,
            Uuid::nil(),
            ImportOptions {
                include_owned_playlists: true,
                include_saved_playlists: false,
                include_followed_artists: false,
                include_saved_tracks: true,
            },
            &PlaylistMappings::default(),
            |_| async {},
            |_, _| async {},
            |_, _, _| async {},
        )
        .await
        .unwrap();

        assert_eq!(outcome.total_items, 5);
        assert_eq!(outcome.imported_items, 3);
        assert_eq!(outcome.unmatched_items, 2);
        assert_eq!(outcome.unmatched_tracks.len(), 2);
        assert_eq!(outcome.unmatched_tracks[0].reason, "not_available_in_tidal");
        assert_eq!(outcome.unmatched_tracks[1].reason, "missing_isrc");
        assert_eq!(
            outcome.unmatched_tracks[0].source_collection,
            "Playlist: owned"
        );
        assert_eq!(progress.playlist_tracks_imported, 1);
        assert_eq!(progress.saved_tracks_imported, 1);
        assert_eq!(progress.tracks_matched, 2);
        assert_eq!(
            *provider.added_playlist_tracks.lock().unwrap(),
            vec![("tidal-owned".to_string(), vec!["tidal-a".to_string()])]
        );
        assert_eq!(
            *provider.saved_tidal_tracks.lock().unwrap(),
            vec!["tidal-b".to_string()]
        );
    }

    #[tokio::test]
    async fn saves_exact_tidal_liked_song_matches_to_spotify_and_audits_misses() {
        let provider = FakeProvider {
            playlists: Vec::new(),
            playlist_tracks: HashMap::new(),
            playlist_track_errors: HashMap::new(),
            tidal_playlists: Vec::new(),
            tidal_playlist_tracks: HashMap::new(),
            saved_tracks: Vec::new(),
            tidal_saved_tracks: vec![
                TidalSavedTrack {
                    isrc: Some("US-AAA-01".to_string()),
                    name: "Matched TIDAL song".to_string(),
                    artist_name: Some("TIDAL artist".to_string()),
                    album_name: Some("TIDAL album".to_string()),
                },
                TidalSavedTrack {
                    isrc: Some("US-MISSING-02".to_string()),
                    name: "Missing TIDAL song".to_string(),
                    artist_name: Some("TIDAL artist".to_string()),
                    album_name: Some("TIDAL album".to_string()),
                },
            ],
            tidal_tracks: HashMap::new(),
            spotify_tracks: HashMap::from([("US-AAA-01".to_string(), "spotify-match".to_string())]),
            artists: Vec::new(),
            matched_artist: None,
            created_playlists: Mutex::new(Vec::new()),
            added_playlist_tracks: Mutex::new(Vec::new()),
            saved_tidal_tracks: Mutex::new(Vec::new()),
            saved_spotify_tracks: Mutex::new(Vec::new()),
            created_spotify_playlists: Mutex::new(Vec::new()),
            added_spotify_playlist_tracks: Mutex::new(Vec::new()),
            followed_artists: Mutex::new(Vec::new()),
        };

        let outcome = execute_import_with_progress(
            &provider,
            Uuid::nil(),
            ImportOptions {
                include_owned_playlists: false,
                include_saved_playlists: false,
                include_followed_artists: false,
                include_saved_tracks: true,
            },
            &PlaylistMappings::default(),
            |_| async {},
            |_, _| async {},
            |_, _, _| async {},
        )
        .await
        .unwrap()
        .0;

        assert_eq!(
            *provider.saved_spotify_tracks.lock().unwrap(),
            vec!["spotify-match".to_string()]
        );
        assert_eq!(outcome.unmatched_tracks.len(), 1);
        assert_eq!(
            outcome.unmatched_tracks[0].reason,
            "not_available_in_spotify"
        );
        assert_eq!(
            outcome.unmatched_tracks[0].source_collection,
            "TIDAL Liked Songs"
        );
    }

    #[tokio::test]
    async fn creates_a_spotify_playlist_for_a_tidal_playlist_with_exact_track_matches() {
        let provider = FakeProvider {
            playlists: Vec::new(),
            playlist_tracks: HashMap::new(),
            playlist_track_errors: HashMap::new(),
            tidal_playlists: vec![TidalPlaylist {
                id: "tidal-playlist".to_string(),
                name: "TIDAL favorites".to_string(),
            }],
            tidal_playlist_tracks: HashMap::from([(
                "tidal-playlist".to_string(),
                vec![track(Some("US-AAA-01")), track(Some("US-MISSING-02"))],
            )]),
            saved_tracks: Vec::new(),
            tidal_saved_tracks: Vec::new(),
            tidal_tracks: HashMap::new(),
            spotify_tracks: HashMap::from([("US-AAA-01".to_string(), "spotify-track".to_string())]),
            artists: Vec::new(),
            matched_artist: None,
            created_playlists: Mutex::new(Vec::new()),
            added_playlist_tracks: Mutex::new(Vec::new()),
            saved_tidal_tracks: Mutex::new(Vec::new()),
            saved_spotify_tracks: Mutex::new(Vec::new()),
            created_spotify_playlists: Mutex::new(Vec::new()),
            added_spotify_playlist_tracks: Mutex::new(Vec::new()),
            followed_artists: Mutex::new(Vec::new()),
        };

        let outcome = execute_import_with_progress(
            &provider,
            Uuid::nil(),
            ImportOptions {
                include_owned_playlists: true,
                include_saved_playlists: false,
                include_followed_artists: false,
                include_saved_tracks: false,
            },
            &PlaylistMappings::default(),
            |_| async {},
            |_, _| async {},
            |_, _, _| async {},
        )
        .await
        .unwrap()
        .0;

        assert_eq!(
            *provider.created_spotify_playlists.lock().unwrap(),
            vec!["TIDAL favorites".to_string()]
        );
        assert_eq!(
            *provider.added_spotify_playlist_tracks.lock().unwrap(),
            vec![(
                "spotify-tidal-playlist".to_string(),
                vec!["spotify-track".to_string()]
            )]
        );
        assert_eq!(outcome.unmatched_tracks.len(), 1);
        assert_eq!(
            outcome.unmatched_tracks[0].reason,
            "not_available_in_spotify"
        );
    }

    #[tokio::test]
    async fn skips_an_inaccessible_spotify_playlist_without_stopping_the_import() {
        let provider = FakeProvider {
            playlists: vec![
                playlist("accessible", "spotify-user", true),
                playlist("inaccessible", "spotify-user", true),
            ],
            playlist_tracks: HashMap::from([(
                "accessible".to_string(),
                vec![track(Some("US-AAA-01"))],
            )]),
            playlist_track_errors: HashMap::from([(
                "inaccessible".to_string(),
                "HTTP status client error (403 Forbidden) for url (https://api.spotify.com/v1/playlists/inaccessible/items?limit=50)".to_string(),
            )]),
            tidal_playlists: Vec::new(),
            tidal_playlist_tracks: HashMap::new(),
            saved_tracks: Vec::new(),
            tidal_saved_tracks: Vec::new(),
            tidal_tracks: HashMap::from([("US-AAA-01".to_string(), "tidal-a".to_string())]),
            spotify_tracks: HashMap::new(),
            artists: Vec::new(),
            matched_artist: None,
            created_playlists: Mutex::new(Vec::new()),
            added_playlist_tracks: Mutex::new(Vec::new()),
            saved_tidal_tracks: Mutex::new(Vec::new()),
            saved_spotify_tracks: Mutex::new(Vec::new()),
            created_spotify_playlists: Mutex::new(Vec::new()),
            added_spotify_playlist_tracks: Mutex::new(Vec::new()),
            followed_artists: Mutex::new(Vec::new()),
        };

        let (outcome, progress) = execute_import_with_progress(
            &provider,
            Uuid::nil(),
            ImportOptions {
                include_owned_playlists: true,
                include_saved_playlists: false,
                include_followed_artists: false,
                include_saved_tracks: false,
            },
            &PlaylistMappings::default(),
            |_| async {},
            |_, _| async {},
            |_, _, _| async {},
        )
        .await
        .unwrap();

        assert_eq!(outcome.total_items, 3);
        assert_eq!(outcome.imported_items, 2);
        assert_eq!(outcome.unmatched_items, 1);
        assert_eq!(
            outcome.warning.as_deref(),
            Some(INACCESSIBLE_SPOTIFY_PLAYLIST_WARNING)
        );
        assert_eq!(progress.playlist_total, 2);
        assert_eq!(progress.playlists_imported, 1);
        assert_eq!(
            *provider.created_playlists.lock().unwrap(),
            vec![("accessible".to_string(), TidalPlaylistVisibility::Public)]
        );
    }

    #[tokio::test]
    async fn reports_collection_specific_import_progress() {
        let provider = FakeProvider {
            playlists: vec![playlist("owned", "spotify-user", true)],
            playlist_tracks: HashMap::new(),
            playlist_track_errors: HashMap::new(),
            tidal_playlists: Vec::new(),
            tidal_playlist_tracks: HashMap::new(),
            saved_tracks: Vec::new(),
            tidal_saved_tracks: Vec::new(),
            tidal_tracks: HashMap::new(),
            spotify_tracks: HashMap::new(),
            artists: vec![SpotifyArtist {
                name: "The Artist".to_string(),
            }],
            matched_artist: Some("tidal-artist".to_string()),
            created_playlists: Mutex::new(Vec::new()),
            added_playlist_tracks: Mutex::new(Vec::new()),
            saved_tidal_tracks: Mutex::new(Vec::new()),
            saved_spotify_tracks: Mutex::new(Vec::new()),
            created_spotify_playlists: Mutex::new(Vec::new()),
            added_spotify_playlist_tracks: Mutex::new(Vec::new()),
            followed_artists: Mutex::new(Vec::new()),
        };
        let updates = Arc::new(Mutex::new(Vec::new()));

        let (_, final_progress) = execute_import_with_progress(
            &provider,
            Uuid::nil(),
            ImportOptions {
                include_owned_playlists: true,
                include_saved_playlists: false,
                include_followed_artists: true,
                include_saved_tracks: false,
            },
            &PlaylistMappings::default(),
            {
                let updates = Arc::clone(&updates);
                move |progress| {
                    let updates = Arc::clone(&updates);
                    async move { updates.lock().unwrap().push(progress) }
                }
            },
            |_, _| async {},
            |_, _, _| async {},
        )
        .await
        .unwrap();

        assert!(updates.lock().unwrap().iter().any(|progress| {
            progress.stage == "creating_playlists"
                && progress.playlist_total == 1
                && progress.artist_total == 1
        }));
        assert_eq!(final_progress.stage, "following_artists");
        assert_eq!(final_progress.playlists_imported, 1);
        assert_eq!(final_progress.artists_checked, 1);
        assert_eq!(final_progress.artists_matched, 1);
        assert_eq!(final_progress.artists_followed, 1);
    }

    #[test]
    fn artist_name_matching_ignores_case_and_whitespace_only() {
        assert!(same_artist_name("The  Artist", " the artist "));
        assert!(!same_artist_name("Artist One", "Artist Two"));
    }

    #[test]
    fn skips_a_playlist_item_that_does_not_match_spotify_track_shape() {
        let item = SpotifyPlaylistTrackItem {
            item: Some(serde_json::json!({
                "type": "episode",
                "name": "Podcast episode",
                "artists": null,
            })),
            track: None,
        };

        assert!(spotify_playlist_item(item).is_none());
    }

    #[test]
    fn restarting_a_run_reuses_its_tidal_idempotency_key() {
        let import_id = Uuid::new_v4();
        assert_eq!(
            idempotency_key(import_id, "playlist:spotify-playlist"),
            idempotency_key(import_id, "playlist:spotify-playlist")
        );
        assert_ne!(
            idempotency_key(import_id, "playlist:spotify-playlist"),
            idempotency_key(Uuid::new_v4(), "playlist:spotify-playlist")
        );
    }

    #[test]
    fn artist_follow_restarts_use_stable_payloads_and_content_specific_keys() {
        let import_id = Uuid::new_v4();
        let mut first = HashSet::new();
        first.insert("artist-c".to_string());
        first.insert("artist-a".to_string());
        first.insert("artist-b".to_string());
        let mut second = HashSet::new();
        second.insert("artist-b".to_string());
        second.insert("artist-c".to_string());
        second.insert("artist-a".to_string());

        let mut first_payload = first.into_iter().collect::<Vec<_>>();
        first_payload.sort_unstable();
        let mut second_payload = second.into_iter().collect::<Vec<_>>();
        second_payload.sort_unstable();

        assert_eq!(first_payload, second_payload);
        assert_eq!(
            artist_follow_idempotency_key(import_id, &first_payload),
            artist_follow_idempotency_key(import_id, &second_payload)
        );
        assert_ne!(
            artist_follow_idempotency_key(import_id, &first_payload),
            idempotency_key(import_id, "artists:0")
        );
        second_payload.push("artist-d".to_string());
        assert_ne!(
            artist_follow_idempotency_key(import_id, &first_payload),
            artist_follow_idempotency_key(import_id, &second_payload)
        );
    }

    #[tokio::test]
    async fn artist_follow_workflow_reuses_identical_payloads_and_keys_after_restart() {
        let provider = |artists: Vec<SpotifyArtist>| FakeProvider {
            playlists: Vec::new(),
            playlist_tracks: HashMap::new(),
            playlist_track_errors: HashMap::new(),
            tidal_playlists: Vec::new(),
            tidal_playlist_tracks: HashMap::new(),
            saved_tracks: Vec::new(),
            tidal_saved_tracks: Vec::new(),
            tidal_tracks: HashMap::new(),
            spotify_tracks: HashMap::new(),
            artists,
            matched_artist: Some("match-by-name".to_string()),
            created_playlists: Mutex::new(Vec::new()),
            added_playlist_tracks: Mutex::new(Vec::new()),
            saved_tidal_tracks: Mutex::new(Vec::new()),
            saved_spotify_tracks: Mutex::new(Vec::new()),
            created_spotify_playlists: Mutex::new(Vec::new()),
            added_spotify_playlist_tracks: Mutex::new(Vec::new()),
            followed_artists: Mutex::new(Vec::new()),
        };
        let artists = (0..90)
            .map(|index| SpotifyArtist {
                name: format!("artist-{index:02}"),
            })
            .collect::<Vec<_>>();
        let first = provider(artists.clone());
        let second = provider(artists.into_iter().rev().collect());
        let import_id = Uuid::new_v4();
        let options = ImportOptions {
            include_owned_playlists: false,
            include_saved_playlists: false,
            include_followed_artists: true,
            include_saved_tracks: false,
        };

        execute_import_with_progress(
            &first,
            import_id,
            options,
            &PlaylistMappings::default(),
            |_| async {},
            |_, _| async {},
            |_, _, _| async {},
        )
        .await
        .unwrap();
        execute_import_with_progress(
            &second,
            import_id,
            options,
            &PlaylistMappings::default(),
            |_| async {},
            |_, _| async {},
            |_, _, _| async {},
        )
        .await
        .unwrap();

        let first_requests = first.followed_artists.lock().unwrap();
        let second_requests = second.followed_artists.lock().unwrap();
        assert_eq!(*first_requests, *second_requests);
        assert_eq!(first_requests.len(), 2);
        assert_eq!(first_requests[0].0.len(), 50);
        assert_eq!(first_requests[1].0.len(), 40);
        assert!(
            first_requests
                .iter()
                .all(|(_, key)| !key.ends_with("artists:0"))
        );
    }

    #[tokio::test]
    async fn follows_tidal_artists_with_the_documented_write_contract() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let app = Router::new().route(
            "/userCollectionArtists/me/relationships/items",
            post({
                let requests = Arc::clone(&requests);
                move |uri: Uri,
                      headers: HeaderMap,
                      axum::Json(body): axum::Json<serde_json::Value>| {
                    let requests = Arc::clone(&requests);
                    async move {
                        requests.lock().unwrap().push((
                            uri.path().to_string(),
                            headers
                                .get(header::AUTHORIZATION)
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_string(),
                            headers
                                .get(header::ACCEPT)
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_string(),
                            headers
                                .get(header::CONTENT_TYPE)
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_string(),
                            headers
                                .get("idempotency-key")
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_string(),
                            body,
                        ));
                        (
                            StatusCode::OK,
                            [(header::CONTENT_TYPE, TIDAL_MEDIA_TYPE)],
                            r#"{"meta":{"skipped":[]}}"#,
                        )
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let provider = HttpMusicImportProvider::with_tidal_api_base(format!("http://{address}"));

        let outcome = provider
            .follow_tidal_artists(
                &["artist-a".to_string(), "artist-b".to_string()],
                "stable-artist-key".to_string(),
            )
            .await
            .unwrap();

        server.abort();
        assert_eq!(
            outcome,
            FollowOutcome {
                imported_items: 2,
                unmatched_items: 0,
            }
        );
        assert_eq!(
            *requests.lock().unwrap(),
            vec![(
                "/userCollectionArtists/me/relationships/items".to_string(),
                "Bearer tidal-access-token".to_string(),
                TIDAL_MEDIA_TYPE.to_string(),
                TIDAL_MEDIA_TYPE.to_string(),
                "stable-artist-key".to_string(),
                serde_json::json!({
                    "data": [
                        {"type": "artists", "id": "artist-a"},
                        {"type": "artists", "id": "artist-b"},
                    ],
                }),
            )]
        );
    }

    #[tokio::test]
    async fn follows_relative_tidal_playlist_pagination_links() {
        let app = Router::new().route(
            "/playlists",
            get(|uri: Uri| async move {
                if uri.query() == Some("cursor=next") {
                    (
                        StatusCode::OK,
                        [(header::CONTENT_TYPE, TIDAL_MEDIA_TYPE)],
                        r#"{"data":[{"id":"second","attributes":{"name":"Second"}}],"links":{}}"#,
                    )
                        .into_response()
                } else {
                    (
                        StatusCode::OK,
                        [(header::CONTENT_TYPE, TIDAL_MEDIA_TYPE)],
                        r#"{"data":[{"id":"first","attributes":{"name":"First"}}],"links":{"next":"/playlists?cursor=next"}}"#,
                    )
                        .into_response()
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let provider = HttpMusicImportProvider::with_tidal_api_base(format!("http://{address}"));

        let playlists = provider.tidal_owned_playlists().await.unwrap();

        server.abort();
        assert_eq!(
            playlists,
            vec![
                TidalPlaylist {
                    id: "first".to_string(),
                    name: "First".to_string(),
                },
                TidalPlaylist {
                    id: "second".to_string(),
                    name: "Second".to_string(),
                },
            ]
        );
    }

    #[tokio::test]
    async fn retries_a_rate_limited_tidal_playlist_request_with_the_same_idempotency_key() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let idempotency_keys = Arc::new(Mutex::new(Vec::new()));
        let media_types = Arc::new(Mutex::new(Vec::new()));
        let app = Router::new().route(
            "/playlists",
            post({
                let attempts = Arc::clone(&attempts);
                let idempotency_keys = Arc::clone(&idempotency_keys);
                let media_types = Arc::clone(&media_types);
                move |headers: HeaderMap| {
                    let attempts = Arc::clone(&attempts);
                    let idempotency_keys = Arc::clone(&idempotency_keys);
                    let media_types = Arc::clone(&media_types);
                    async move {
                        idempotency_keys.lock().unwrap().push(
                            headers
                                .get("idempotency-key")
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_string(),
                        );
                        media_types.lock().unwrap().push((
                            headers
                                .get(header::ACCEPT)
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_string(),
                            headers
                                .get(header::CONTENT_TYPE)
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_string(),
                        ));
                        if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                            (StatusCode::TOO_MANY_REQUESTS, [(header::RETRY_AFTER, "0")])
                                .into_response()
                        } else {
                            (
                                StatusCode::CREATED,
                                [(header::CONTENT_TYPE, TIDAL_MEDIA_TYPE)],
                                r#"{"data":{"type":"playlists","id":"tidal-playlist"}}"#,
                            )
                                .into_response()
                        }
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let provider = HttpMusicImportProvider::with_tidal_api_base(format!("http://{address}"));

        provider
            .create_tidal_playlist(
                &playlist("playlist", "spotify-user", true),
                TidalPlaylistVisibility::Public,
                "stable-idempotency-key".to_string(),
            )
            .await
            .unwrap();

        server.abort();
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert_eq!(
            *idempotency_keys.lock().unwrap(),
            vec!["stable-idempotency-key", "stable-idempotency-key"]
        );
        assert_eq!(
            *media_types.lock().unwrap(),
            vec![
                (
                    "application/vnd.api+json".to_string(),
                    "application/vnd.api+json".to_string(),
                ),
                (
                    "application/vnd.api+json".to_string(),
                    "application/vnd.api+json".to_string(),
                ),
            ]
        );
    }

    #[tokio::test]
    async fn retries_a_rate_limited_spotify_request_after_retry_after() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let authorizations = Arc::new(Mutex::new(Vec::new()));
        let app = Router::new().route(
            "/spotify-search",
            get({
                let attempts = Arc::clone(&attempts);
                let authorizations = Arc::clone(&authorizations);
                move |headers: HeaderMap| {
                    let attempts = Arc::clone(&attempts);
                    let authorizations = Arc::clone(&authorizations);
                    async move {
                        authorizations.lock().unwrap().push(
                            headers
                                .get(header::AUTHORIZATION)
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_string(),
                        );
                        if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                            (StatusCode::TOO_MANY_REQUESTS, [(header::RETRY_AFTER, "0")])
                                .into_response()
                        } else {
                            StatusCode::OK.into_response()
                        }
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let provider = HttpMusicImportProvider::with_tidal_api_base("http://unused".to_string());
        let url = format!("http://{address}/spotify-search");

        provider
            .spotify_request(|| {
                provider
                    .client
                    .get(&url)
                    .bearer_auth(&provider.spotify_access_token)
            })
            .await
            .unwrap();

        server.abort();
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert_eq!(
            *authorizations.lock().unwrap(),
            vec![
                "Bearer spotify-access-token".to_string(),
                "Bearer spotify-access-token".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn adds_spotify_playlist_tracks_with_the_documented_post_contract() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let app = Router::new().route(
            "/v1/playlists/playlist-id/items",
            post({
                let requests = Arc::clone(&requests);
                move |uri: Uri,
                      headers: HeaderMap,
                      axum::Json(body): axum::Json<serde_json::Value>| {
                    let requests = Arc::clone(&requests);
                    async move {
                        requests.lock().unwrap().push((
                            uri.path().to_string(),
                            headers
                                .get(header::AUTHORIZATION)
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_string(),
                            headers
                                .get(header::CONTENT_TYPE)
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_string(),
                            body,
                        ));
                        (
                            StatusCode::CREATED,
                            axum::Json(serde_json::json!({"snapshot_id": "snapshot"})),
                        )
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let provider = HttpMusicImportProvider::with_spotify_api_base(format!("http://{address}"));
        let track_ids = (0..101)
            .map(|index| format!("track-{index}"))
            .collect::<Vec<_>>();

        provider
            .add_spotify_playlist_tracks("playlist-id", &track_ids)
            .await
            .unwrap();

        server.abort();
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests.iter().all(|request| {
            request.0 == "/v1/playlists/playlist-id/items"
                && request.1 == "Bearer spotify-access-token"
                && request.2.starts_with("application/json")
        }));
        assert_eq!(requests[0].3["uris"].as_array().unwrap().len(), 100);
        assert_eq!(requests[1].3["uris"].as_array().unwrap().len(), 1);
        assert_eq!(requests[0].3["uris"][0], "spotify:track:track-0");
        assert_eq!(requests[1].3["uris"][0], "spotify:track:track-100");
    }

    #[tokio::test]
    async fn defers_a_long_spotify_rate_limit_without_sleeping_in_the_worker() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let app = Router::new().route(
            "/spotify-search",
            get({
                let attempts = Arc::clone(&attempts);
                move || {
                    let attempts = Arc::clone(&attempts);
                    async move {
                        attempts.fetch_add(1, Ordering::SeqCst);
                        (
                            StatusCode::TOO_MANY_REQUESTS,
                            [(header::RETRY_AFTER, "3600")],
                        )
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let provider = HttpMusicImportProvider::with_tidal_api_base("http://unused".to_string());
        let url = format!("http://{address}/spotify-search");

        let error = tokio::time::timeout(
            Duration::from_secs(1),
            provider.spotify_request(|| {
                provider
                    .client
                    .get(&url)
                    .bearer_auth(&provider.spotify_access_token)
            }),
        )
        .await
        .expect("long provider retry must be returned to the task scheduler")
        .unwrap_err();

        server.abort();
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        assert_eq!(
            deferred_rate_limit_delay(&error),
            Some(Duration::from_secs(3600))
        );
    }

    #[tokio::test]
    async fn schedules_persistent_short_spotify_rate_limits_after_bounded_retries() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let app = Router::new().route(
            "/spotify-search",
            get({
                let attempts = Arc::clone(&attempts);
                move || {
                    let attempts = Arc::clone(&attempts);
                    async move {
                        attempts.fetch_add(1, Ordering::SeqCst);
                        (StatusCode::TOO_MANY_REQUESTS, [(header::RETRY_AFTER, "0")])
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let provider = HttpMusicImportProvider::with_tidal_api_base("http://unused".to_string());
        let url = format!("http://{address}/spotify-search");

        let error = provider
            .spotify_request(|| {
                provider
                    .client
                    .get(&url)
                    .bearer_auth(&provider.spotify_access_token)
            })
            .await
            .unwrap_err();

        server.abort();
        assert_eq!(
            attempts.load(Ordering::SeqCst),
            SPOTIFY_RATE_LIMIT_RETRIES as usize + 1
        );
        assert_eq!(
            deferred_rate_limit_delay(&error),
            Some(Duration::from_secs(1))
        );
    }

    #[test]
    fn tidal_requests_use_the_json_api_media_type() {
        assert_eq!(TIDAL_MEDIA_TYPE, "application/vnd.api+json");
    }

    #[tokio::test]
    async fn appends_playlist_tracks_without_a_position_before_uuid() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let app = Router::new().route(
            "/playlists/tidal-playlist/relationships/items",
            post({
                let requests = Arc::clone(&requests);
                move |body: String| {
                    let requests = Arc::clone(&requests);
                    async move {
                        requests
                            .lock()
                            .unwrap()
                            .push(serde_json::from_str::<serde_json::Value>(&body).unwrap());
                        (
                            StatusCode::OK,
                            [(header::CONTENT_TYPE, TIDAL_MEDIA_TYPE)],
                            r#"{"data":[],"links":{},"meta":{"skipped":[]}}"#,
                        )
                            .into_response()
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let provider = HttpMusicImportProvider::with_tidal_api_base(format!("http://{address}"));

        let result = provider
            .add_tidal_playlist_tracks(
                "tidal-playlist",
                &["tidal-track-a".to_string(), "tidal-track-b".to_string()],
                "stable-idempotency-key".to_string(),
            )
            .await
            .unwrap();

        server.abort();
        assert_eq!(result.imported_items, 2);
        assert_eq!(
            *requests.lock().unwrap(),
            vec![serde_json::json!({
                "data": [
                    { "type": "tracks", "id": "tidal-track-a" },
                    { "type": "tracks", "id": "tidal-track-b" },
                ],
            })]
        );
    }

    #[tokio::test]
    async fn existing_tidal_tracks_are_reconciled_without_being_counted_as_unmatched() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let app = Router::new().route(
            "/userCollectionTracks/me/relationships/items",
            post({
                let requests = Arc::clone(&requests);
                move |headers: HeaderMap, body: String| {
                    let requests = Arc::clone(&requests);
                    async move {
                        requests.lock().unwrap().push((
                            headers
                                .get(header::CONTENT_TYPE)
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_string(),
                            headers
                                .get("idempotency-key")
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_string(),
                            serde_json::from_str::<serde_json::Value>(&body).unwrap(),
                        ));
                        (
                            StatusCode::OK,
                            [(header::CONTENT_TYPE, TIDAL_MEDIA_TYPE)],
                            r#"{
                                "data": [],
                                "links": {},
                                "meta": {
                                    "skipped": [
                                        {"id":"already-present","type":"tracks","reason":"ALREADY_PRESENT"},
                                        {"id":"not-found","type":"tracks","reason":"NOT_FOUND"}
                                    ]
                                }
                            }"#,
                        )
                            .into_response()
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let provider = HttpMusicImportProvider::with_tidal_api_base(format!("http://{address}"));

        let result = provider
            .save_tidal_tracks(
                &[
                    "already-present".to_string(),
                    "not-found".to_string(),
                    "new-favorite".to_string(),
                ],
                "stable-idempotency-key".to_string(),
            )
            .await
            .unwrap();

        server.abort();
        assert_eq!(
            result,
            TrackWriteOutcome {
                imported_items: 1,
                unmatched_items: 1,
            }
        );
        assert_eq!(
            *requests.lock().unwrap(),
            vec![(
                TIDAL_MEDIA_TYPE.to_string(),
                "stable-idempotency-key".to_string(),
                serde_json::json!({
                    "data": [
                        {"type":"tracks","id":"already-present"},
                        {"type":"tracks","id":"not-found"},
                        {"type":"tracks","id":"new-favorite"},
                    ]
                }),
            )]
        );
    }

    #[tokio::test]
    async fn saves_non_duplicate_liked_tracks_when_tidal_rejects_a_mixed_batch() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let app = Router::new().route(
            "/userCollectionTracks/me/relationships/items",
            post({
                let requests = Arc::clone(&requests);
                move |body: String| {
                    let requests = Arc::clone(&requests);
                    async move {
                        let body = serde_json::from_str::<serde_json::Value>(&body).unwrap();
                        let ids = body["data"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|item| item["id"].as_str().unwrap().to_string())
                            .collect::<Vec<_>>();
                        requests.lock().unwrap().push(ids.clone());
                        if ids.iter().any(|id| id == "already-favorited") {
                            (
                                StatusCode::CONFLICT,
                                [(header::CONTENT_TYPE, TIDAL_MEDIA_TYPE)],
                                r#"{"errors":[{"code":"DUPLICATE_ITEMS_IN_COLLECTION","detail":"already favorited"}]}"#,
                            )
                                .into_response()
                        } else {
                            (
                                StatusCode::OK,
                                [(header::CONTENT_TYPE, TIDAL_MEDIA_TYPE)],
                                r#"{"data":[],"meta":{"skipped":[]}}"#,
                            )
                                .into_response()
                        }
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let provider = HttpMusicImportProvider::with_tidal_api_base(format!("http://{address}"));

        let result = provider
            .save_tidal_tracks(
                &["already-favorited".to_string(), "new-favorite".to_string()],
                "stable-idempotency-key".to_string(),
            )
            .await
            .unwrap();

        server.abort();
        assert_eq!(result.imported_items, 1);
        assert_eq!(result.unmatched_items, 0);
        assert_eq!(
            *requests.lock().unwrap(),
            vec![
                vec!["already-favorited".to_string(), "new-favorite".to_string()],
                vec!["already-favorited".to_string()],
                vec!["new-favorite".to_string()],
            ]
        );
    }

    #[tokio::test]
    async fn looks_up_all_isrcs_with_repeated_array_filters() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let app = Router::new().route(
            "/tracks",
            get({
                let requests = Arc::clone(&requests);
                move |uri: Uri, headers: HeaderMap| {
                    let requests = Arc::clone(&requests);
                    async move {
                        requests.lock().unwrap().push((
                            uri.query().unwrap_or_default().to_string(),
                            headers
                                .get("accept")
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_string(),
                        ));
                        (
                            StatusCode::OK,
                            [(header::CONTENT_TYPE, TIDAL_MEDIA_TYPE)],
                            r#"{"data":[{"id":"tidal-a","attributes":{"isrc":"us-aaa-01"}},{"id":"tidal-b","attributes":{"isrc":"US-BBB-02"}}]}"#,
                        )
                            .into_response()
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let provider = HttpMusicImportProvider::with_tidal_api_base(format!("http://{address}"));

        let matches = provider
            .tidal_tracks_by_isrc(&["US-AAA-01".to_string(), "US-BBB-02".to_string()])
            .await
            .unwrap();

        server.abort();
        assert_eq!(
            *requests.lock().unwrap(),
            vec![(
                "filter%5Bisrc%5D=US-AAA-01&filter%5Bisrc%5D=US-BBB-02".to_string(),
                TIDAL_MEDIA_TYPE.to_string(),
            )]
        );
        assert_eq!(
            matches,
            HashMap::from([
                ("US-AAA-01".to_string(), "tidal-a".to_string()),
                ("US-BBB-02".to_string(), "tidal-b".to_string()),
            ])
        );
    }

    #[tokio::test]
    async fn searches_tidal_by_title_and_verifies_the_exact_artist() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let app = Router::new()
            .route(
                "/searchResults/{query}",
                get({
                    let requests = Arc::clone(&requests);
                    move |uri: Uri, headers: HeaderMap| {
                        let requests = Arc::clone(&requests);
                        async move {
                            requests.lock().unwrap().push((
                                uri.path().to_string(),
                                uri.query().unwrap_or_default().to_string(),
                                headers.get("accept").unwrap().to_str().unwrap().to_string(),
                            ));
                            (
                                StatusCode::OK,
                                [(header::CONTENT_TYPE, TIDAL_MEDIA_TYPE)],
                                r#"{"data":{"id":"First Day of My Life Bright Eyes","type":"searchResults"},"included":[{"id":"tidal-candidate","type":"tracks","attributes":{"title":"First Day of My Life"}}]}"#,
                            )
                                .into_response()
                        }
                    }
                }),
            )
            .route(
                "/tracks",
                get({
                    let requests = Arc::clone(&requests);
                    move |uri: Uri, headers: HeaderMap| {
                        let requests = Arc::clone(&requests);
                        async move {
                            requests.lock().unwrap().push((
                                uri.path().to_string(),
                                uri.query().unwrap_or_default().to_string(),
                                headers.get("accept").unwrap().to_str().unwrap().to_string(),
                            ));
                            (
                                StatusCode::OK,
                                [(header::CONTENT_TYPE, TIDAL_MEDIA_TYPE)],
                                r#"{"data":[{"id":"tidal-candidate","type":"tracks","attributes":{"title":"First Day of My Life"},"relationships":{"artists":{"data":[{"id":"bright-eyes","type":"artists"}]}}}],"included":[{"id":"bright-eyes","type":"artists","attributes":{"name":"Bright Eyes"}}]}"#,
                            )
                                .into_response()
                        }
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let provider = HttpMusicImportProvider::with_tidal_api_base(format!("http://{address}"));
        let track = SpotifyTrack {
            source_id: Some("spotify-track".to_string()),
            isrc: Some("US-SPOTIFY-ISRC".to_string()),
            name: "First Day of My Life".to_string(),
            artist_name: Some("Bright Eyes".to_string()),
            album_name: Some("I'm Wide Awake, It's Morning".to_string()),
        };

        let matched = provider.find_tidal_track(&track).await.unwrap();

        server.abort();
        assert_eq!(matched, Some("tidal-candidate".to_string()));
        assert_eq!(
            *requests.lock().unwrap(),
            vec![
                (
                    "/searchResults/First%20Day%20of%20My%20Life%20Bright%20Eyes".to_string(),
                    "include=tracks".to_string(),
                    TIDAL_MEDIA_TYPE.to_string(),
                ),
                (
                    "/tracks".to_string(),
                    "filter%5Bid%5D=tidal-candidate&include=artists".to_string(),
                    TIDAL_MEDIA_TYPE.to_string(),
                ),
            ]
        );
    }

    #[tokio::test]
    async fn splits_isrc_lookups_at_tidal_filter_limit() {
        let queries = Arc::new(Mutex::new(Vec::new()));
        let app = Router::new().route(
            "/tracks",
            get({
                let queries = Arc::clone(&queries);
                move |uri: Uri| {
                    let queries = Arc::clone(&queries);
                    async move {
                        queries
                            .lock()
                            .unwrap()
                            .push(uri.query().unwrap_or_default().to_string());
                        (
                            StatusCode::OK,
                            [(header::CONTENT_TYPE, TIDAL_MEDIA_TYPE)],
                            r#"{"data":[]}"#,
                        )
                            .into_response()
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let provider = HttpMusicImportProvider::with_tidal_api_base(format!("http://{address}"));
        let isrcs = (0..=TIDAL_ISRC_FILTER_LIMIT)
            .map(|index| format!("US-TEST-{index:02}"))
            .collect::<Vec<_>>();

        provider.tidal_tracks_by_isrc(&isrcs).await.unwrap();

        server.abort();
        let queries = queries.lock().unwrap();
        assert_eq!(queries.len(), 2);
        assert_eq!(
            queries[0].matches("filter%5Bisrc%5D=").count(),
            TIDAL_ISRC_FILTER_LIMIT
        );
        assert_eq!(queries[1].matches("filter%5Bisrc%5D=").count(), 1);
    }

    #[tokio::test]
    async fn tidal_errors_keep_safe_provider_detail_without_query_values() {
        let app = Router::new().route(
            "/tracks",
            get(|| async {
                (
                    StatusCode::BAD_REQUEST,
                    [(header::CONTENT_TYPE, TIDAL_MEDIA_TYPE)],
                    r#"{"errors":[{"code":"VALIDATION_ERROR","detail":"filter[isrc] must use repeated values"}]}"#,
                )
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let provider = HttpMusicImportProvider::with_tidal_api_base(format!("http://{address}"));

        let error = provider
            .tidal_tracks_by_isrc(&["SECRET-ISRC".to_string()])
            .await
            .unwrap_err();

        server.abort();
        assert_eq!(
            error,
            "TIDAL returned 400 Bad Request for /tracks: VALIDATION_ERROR: filter[isrc] must use repeated values"
        );
        assert!(!error.contains("SECRET-ISRC"));
    }

    #[test]
    fn provider_error_diagnostic_keeps_structured_tidal_error_fields() {
        let (detail, diagnostic) = provider_error_diagnostic(
            r#"{"errors":[{"code":"MISSING_SCOPE","status":"403","detail":"playlists.write is required"}]}"#,
        );

        assert_eq!(
            detail.as_deref(),
            Some("MISSING_SCOPE: playlists.write is required")
        );
        assert_eq!(
            diagnostic,
            "code=MISSING_SCOPE status=403 detail=playlists.write is required"
        );
    }
}
