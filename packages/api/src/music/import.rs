use std::{
    collections::{HashMap, HashSet},
    future::Future,
    sync::OnceLock,
    time::{Duration, Instant},
};

use reqwest::{Client, Response, StatusCode};
use serde::Deserialize;
use tokio::{sync::Mutex, time::sleep};
use uuid::Uuid;

use super::query_value;

const SPOTIFY_API_BASE: &str = "https://api.spotify.com";
const TIDAL_API_BASE: &str = "https://openapi.tidal.com/v2";
const TIDAL_MEDIA_TYPE: &str = "application/vnd.api+json";
const TIDAL_REQUEST_INTERVAL: Duration = Duration::from_secs(1);
const TIDAL_RATE_LIMIT_RETRIES: u8 = 3;
const TIDAL_ISRC_FILTER_LIMIT: usize = 20;
const TIDAL_DUPLICATE_COLLECTION_ITEMS: &str = "DUPLICATE_ITEMS_IN_COLLECTION";
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

fn tidal_retry_after(response: &Response, retry: u8) -> Duration {
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
    pub isrc: Option<String>,
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ImportProgress {
    pub stage: &'static str,
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
    async fn spotify_followed_artists(&self) -> Result<Vec<SpotifyArtist>, String>;
    async fn create_tidal_playlist(
        &self,
        playlist: &SpotifyPlaylist,
        visibility: TidalPlaylistVisibility,
        idempotency_key: String,
    ) -> Result<String, String>;
    async fn tidal_tracks_by_isrc(
        &self,
        isrcs: &[String],
    ) -> Result<HashMap<String, String>, String>;
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
    tidal_api_base: String,
}

impl HttpMusicImportProvider {
    pub(super) fn new(spotify_access_token: String, tidal_access_token: String) -> Self {
        Self {
            client: Client::new(),
            spotify_access_token,
            tidal_access_token,
            tidal_api_base: TIDAL_API_BASE.to_string(),
        }
    }

    #[cfg(test)]
    fn with_tidal_api_base(tidal_api_base: String) -> Self {
        Self {
            client: Client::new(),
            spotify_access_token: "spotify-access-token".to_string(),
            tidal_access_token: "tidal-access-token".to_string(),
            tidal_api_base,
        }
    }

    async fn spotify_get<T: for<'de> Deserialize<'de>>(&self, url: String) -> Result<T, String> {
        let response = self
            .client
            .get(url)
            .bearer_auth(&self.spotify_access_token)
            .send()
            .await
            .map_err(|error| error.to_string())?;
        if !response.status().is_success() {
            return Err(provider_http_error("Spotify", response).await);
        }
        response.json().await.map_err(|error| error.to_string())
    }

    async fn tidal_get<T: for<'de> Deserialize<'de>>(&self, url: String) -> Result<T, String> {
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

    async fn tidal_request<F>(&self, request: F) -> Result<Response, String>
    where
        F: Fn() -> reqwest::RequestBuilder,
    {
        for retry in 0..=TIDAL_RATE_LIMIT_RETRIES {
            wait_for_tidal_request_slot().await;
            let response = request().send().await.map_err(|error| error.to_string())?;
            if response.status() != StatusCode::TOO_MANY_REQUESTS {
                return if response.status().is_success() {
                    Ok(response)
                } else {
                    Err(provider_http_error("TIDAL", response).await)
                };
            }

            if retry == TIDAL_RATE_LIMIT_RETRIES {
                return Err("TIDAL is temporarily rate limiting imports; please wait and restart this import".to_string());
            }
            let delay = tidal_retry_after(&response, retry);
            tracing::warn!(
                retry,
                retry_after_seconds = delay.as_secs(),
                "TIDAL rate limited music import request"
            );
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
        let skipped = response.meta.skipped.len() as i32;
        Ok(TrackWriteOutcome {
            imported_items: track_ids.len() as i32 - skipped,
            unmatched_items: skipped,
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
    async fn spotify_current_user_id(&self) -> Result<String, String> {
        Ok(self
            .spotify_get::<SpotifyCurrentUser>(format!("{SPOTIFY_API_BASE}/v1/me"))
            .await?
            .id)
    }

    async fn spotify_playlists(&self) -> Result<Vec<SpotifyPlaylist>, String> {
        let mut url = format!("{SPOTIFY_API_BASE}/v1/me/playlists?limit=50");
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
            "{SPOTIFY_API_BASE}/v1/playlists/{}/items?limit=50",
            query_value(playlist_id)
        );
        let mut tracks = Vec::new();
        loop {
            let page: SpotifyPlaylistTrackPage = self.spotify_get(url).await?;
            tracks.extend(
                page.items
                    .into_iter()
                    .filter_map(|item| item.track)
                    .filter_map(spotify_track),
            );
            match page.next {
                Some(next) => url = next,
                None => return Ok(tracks),
            }
        }
    }

    async fn spotify_saved_tracks(&self) -> Result<Vec<SpotifyTrack>, String> {
        let mut url = format!("{SPOTIFY_API_BASE}/v1/me/tracks?limit=50");
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

    async fn spotify_followed_artists(&self) -> Result<Vec<SpotifyArtist>, String> {
        let mut url = format!("{SPOTIFY_API_BASE}/v1/me/following?type=artist&limit=50");
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
        let mut matches = HashMap::new();
        for isrcs in isrcs.chunks(TIDAL_ISRC_FILTER_LIMIT) {
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
        Ok(matches)
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

pub(super) async fn execute_import_with_progress<P, F, Fut>(
    provider: &P,
    import_id: Uuid,
    options: ImportOptions,
    mut report_progress: F,
) -> Result<(ImportOutcome, ImportProgress), ImportFailure>
where
    P: MusicImportProvider,
    F: FnMut(ImportProgress) -> Fut,
    Fut: Future<Output = ()>,
{
    let mut outcome = ImportOutcome::default();
    let mut progress = ImportProgress {
        stage: "reading_spotify",
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
    report_progress(progress.clone()).await;

    for (playlist, tracks) in playlist_tracks {
        let visibility = if playlist.is_public {
            TidalPlaylistVisibility::Public
        } else {
            TidalPlaylistVisibility::Unlisted
        };
        let tidal_playlist_id = provider
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
        outcome.imported_items += 1;
        progress.playlists_imported += 1;
        report_progress(progress.clone()).await;

        progress.stage = "adding_playlist_tracks";
        report_progress(progress.clone()).await;
        for (batch_index, tracks) in tracks.chunks(50).enumerate() {
            let (tidal_track_ids, unmatched) =
                match_tidal_tracks(provider, tracks)
                    .await
                    .map_err(|message| ImportFailure {
                        message,
                        outcome: outcome.clone(),
                        progress: progress.clone(),
                    })?;
            outcome.unmatched_items += unmatched;
            progress.tracks_unmatched += unmatched;
            progress.tracks_matched += tidal_track_ids.len() as i32;
            if !tidal_track_ids.is_empty() {
                let result = provider
                    .add_tidal_playlist_tracks(
                        &tidal_playlist_id,
                        &tidal_track_ids,
                        idempotency_key(
                            import_id,
                            &format!("playlist_tracks:{}:{batch_index}", playlist.id),
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
            }
            report_progress(progress.clone()).await;
        }
    }

    progress.stage = "matching_artists";
    report_progress(progress.clone()).await;
    let mut tidal_artist_ids = HashSet::new();
    let artist_count = artists.len();
    for (index, artist) in artists.iter().enumerate() {
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
    let tidal_artist_ids = tidal_artist_ids.into_iter().collect::<Vec<_>>();
    progress.stage = "following_artists";
    report_progress(progress.clone()).await;
    for (batch_index, artist_ids) in tidal_artist_ids.chunks(50).enumerate() {
        let result = provider
            .follow_tidal_artists(
                artist_ids,
                idempotency_key(import_id, &format!("artists:{batch_index}")),
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

    if !saved_tracks.is_empty() {
        progress.stage = "saving_liked_tracks";
        report_progress(progress.clone()).await;
        for (batch_index, tracks) in saved_tracks.chunks(50).enumerate() {
            let (tidal_track_ids, unmatched) =
                match_tidal_tracks(provider, tracks)
                    .await
                    .map_err(|message| ImportFailure {
                        message,
                        outcome: outcome.clone(),
                        progress: progress.clone(),
                    })?;
            outcome.unmatched_items += unmatched;
            progress.tracks_unmatched += unmatched;
            progress.tracks_matched += tidal_track_ids.len() as i32;
            if !tidal_track_ids.is_empty() {
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
    }
    Ok((outcome, progress))
}

fn is_inaccessible_spotify_playlist_error(message: &str) -> bool {
    message.contains("403 Forbidden")
}

async fn match_tidal_tracks<P>(
    provider: &P,
    tracks: &[SpotifyTrack],
) -> Result<(Vec<String>, i32), String>
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
    let tidal_track_ids = tracks
        .iter()
        .filter_map(|track| track.isrc.as_ref())
        .filter_map(|isrc| tidal_tracks.get(isrc))
        .cloned()
        .collect::<Vec<_>>();
    let unmatched = tracks.len() as i32 - tidal_track_ids.len() as i32;
    Ok((tidal_track_ids, unmatched))
}

fn idempotency_key(import_id: Uuid, purpose: &str) -> String {
    format!("{import_id}-{purpose}")
}

fn same_artist_name(left: &str, right: &str) -> bool {
    normalize_artist_name(left) == normalize_artist_name(right)
}

fn normalize_artist_name(value: &str) -> String {
    value.split_whitespace().map(str::to_lowercase).collect()
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
    #[serde(alias = "item")]
    track: Option<SpotifyTrackWire>,
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
    #[serde(rename = "type")]
    resource_type: Option<String>,
    #[serde(default)]
    is_local: bool,
    external_ids: Option<SpotifyExternalIds>,
}

#[derive(Deserialize)]
struct SpotifyExternalIds {
    isrc: Option<String>,
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
            isrc: track
                .external_ids
                .and_then(|ids| ids.isrc)
                .map(|isrc| normalize_isrc(&isrc)),
        },
    )
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
struct TidalTracksResponse {
    data: Vec<TidalTrackResource>,
}

#[derive(Deserialize)]
struct TidalTrackResource {
    id: String,
    attributes: TidalTrackAttributes,
}

#[derive(Deserialize)]
struct TidalTrackAttributes {
    isrc: Option<String>,
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
    skipped: Vec<serde_json::Value>,
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

    struct FakeProvider {
        playlists: Vec<SpotifyPlaylist>,
        playlist_tracks: HashMap<String, Vec<SpotifyTrack>>,
        playlist_track_errors: HashMap<String, String>,
        saved_tracks: Vec<SpotifyTrack>,
        tidal_tracks: HashMap<String, String>,
        artists: Vec<SpotifyArtist>,
        matched_artist: Option<String>,
        created_playlists: Mutex<Vec<(String, TidalPlaylistVisibility)>>,
        added_playlist_tracks: Mutex<Vec<(String, Vec<String>)>>,
        saved_tidal_tracks: Mutex<Vec<String>>,
        followed_artists: Mutex<Vec<String>>,
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
            Ok(TrackWriteOutcome {
                imported_items: track_ids.len() as i32,
                unmatched_items: 0,
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

        async fn find_tidal_artist(&self, _name: &str) -> Result<Option<String>, String> {
            Ok(self.matched_artist.clone())
        }

        async fn follow_tidal_artists(
            &self,
            artist_ids: &[String],
            _idempotency_key: String,
        ) -> Result<FollowOutcome, String> {
            self.followed_artists
                .lock()
                .unwrap()
                .extend_from_slice(artist_ids);
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
            isrc: isrc.map(str::to_string),
        }
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
            saved_tracks: Vec::new(),
            tidal_tracks: HashMap::new(),
            artists: vec![SpotifyArtist {
                name: "The Artist".to_string(),
            }],
            matched_artist: Some("tidal-artist".to_string()),
            created_playlists: Mutex::new(Vec::new()),
            added_playlist_tracks: Mutex::new(Vec::new()),
            saved_tidal_tracks: Mutex::new(Vec::new()),
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
            |_| async {},
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
            *provider.followed_artists.lock().unwrap(),
            vec!["tidal-artist".to_string()]
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
            saved_tracks: Vec::new(),
            tidal_tracks: HashMap::new(),
            artists: Vec::new(),
            matched_artist: None,
            created_playlists: Mutex::new(Vec::new()),
            added_playlist_tracks: Mutex::new(Vec::new()),
            saved_tidal_tracks: Mutex::new(Vec::new()),
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
            |_| async {},
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
            saved_tracks: vec![track(Some("US-BBB-03"))],
            tidal_tracks: HashMap::from([
                ("US-AAA-01".to_string(), "tidal-a".to_string()),
                ("US-BBB-03".to_string(), "tidal-b".to_string()),
            ]),
            artists: Vec::new(),
            matched_artist: None,
            created_playlists: Mutex::new(Vec::new()),
            added_playlist_tracks: Mutex::new(Vec::new()),
            saved_tidal_tracks: Mutex::new(Vec::new()),
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
            |_| async {},
        )
        .await
        .unwrap();

        assert_eq!(outcome.total_items, 5);
        assert_eq!(outcome.imported_items, 3);
        assert_eq!(outcome.unmatched_items, 2);
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
            saved_tracks: Vec::new(),
            tidal_tracks: HashMap::from([("US-AAA-01".to_string(), "tidal-a".to_string())]),
            artists: Vec::new(),
            matched_artist: None,
            created_playlists: Mutex::new(Vec::new()),
            added_playlist_tracks: Mutex::new(Vec::new()),
            saved_tidal_tracks: Mutex::new(Vec::new()),
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
            |_| async {},
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
            saved_tracks: Vec::new(),
            tidal_tracks: HashMap::new(),
            artists: vec![SpotifyArtist {
                name: "The Artist".to_string(),
            }],
            matched_artist: Some("tidal-artist".to_string()),
            created_playlists: Mutex::new(Vec::new()),
            added_playlist_tracks: Mutex::new(Vec::new()),
            saved_tidal_tracks: Mutex::new(Vec::new()),
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
            {
                let updates = Arc::clone(&updates);
                move |progress| {
                    let updates = Arc::clone(&updates);
                    async move { updates.lock().unwrap().push(progress) }
                }
            },
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
