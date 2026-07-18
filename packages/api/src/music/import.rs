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
const TIDAL_REQUEST_INTERVAL: Duration = Duration::from_secs(1);
const TIDAL_RATE_LIMIT_RETRIES: u8 = 3;

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
        self.client
            .get(url)
            .bearer_auth(&self.spotify_access_token)
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?
            .json()
            .await
            .map_err(|error| error.to_string())
    }

    async fn tidal_get<T: for<'de> Deserialize<'de>>(&self, url: String) -> Result<T, String> {
        self.tidal_request(|| {
            self.client
                .get(&url)
                .bearer_auth(&self.tidal_access_token)
                .header("accept", "application/vnd.api+json")
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
                return response
                    .error_for_status()
                    .map_err(|error| error.to_string());
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
                    .header("accept", "application/vnd.api+json")
                    .header("content-type", "application/vnd.api+json")
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
            "{SPOTIFY_API_BASE}/v1/playlists/{}/tracks?limit=50",
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
                .header("accept", "application/vnd.api+json")
                .header("content-type", "application/vnd.api+json")
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
        let query = isrcs
            .iter()
            .map(|isrc| format!("filter%5Bisrc%5D={}", query_value(isrc)))
            .collect::<Vec<_>>()
            .join("&");
        let response: TidalTracksResponse = self
            .tidal_get(format!("{}/tracks?{query}", self.tidal_api_base))
            .await?;
        Ok(response
            .data
            .into_iter()
            .filter_map(|track| track.attributes.isrc.map(|isrc| (isrc, track.id)))
            .collect())
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
            Some("end"),
        )
        .await
    }

    async fn save_tidal_tracks(
        &self,
        track_ids: &[String],
        idempotency_key: String,
    ) -> Result<TrackWriteOutcome, String> {
        self.tidal_add_tracks(
            format!(
                "{}/userCollectionTracks/me/relationships/items",
                self.tidal_api_base
            ),
            track_ids,
            idempotency_key,
            None,
        )
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
                    .header("accept", "application/vnd.api+json")
                    .header("content-type", "application/vnd.api+json")
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
    for playlist in playlists {
        let tracks = provider
            .spotify_playlist_tracks(&playlist.id)
            .await
            .map_err(|message| ImportFailure {
                message,
                outcome: outcome.clone(),
                progress: progress.clone(),
            })?;
        playlist_tracks.push((playlist, tracks));
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
    outcome.total_items = (playlist_tracks.len()
        + artists.len()
        + playlist_tracks
            .iter()
            .map(|(_, tracks)| tracks.len())
            .sum::<usize>()
        + saved_tracks.len()) as i32;
    progress.playlist_total = playlist_tracks.len() as i32;
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
            isrc: track.external_ids.and_then(|ids| ids.isrc),
        },
    )
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
        http::{HeaderMap, StatusCode, header},
        response::IntoResponse,
        routing::post,
    };

    struct FakeProvider {
        playlists: Vec<SpotifyPlaylist>,
        playlist_tracks: HashMap<String, Vec<SpotifyTrack>>,
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
    async fn reports_collection_specific_import_progress() {
        let provider = FakeProvider {
            playlists: vec![playlist("owned", "spotify-user", true)],
            playlist_tracks: HashMap::new(),
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
        let app = Router::new().route(
            "/playlists",
            post({
                let attempts = Arc::clone(&attempts);
                let idempotency_keys = Arc::clone(&idempotency_keys);
                move |headers: HeaderMap| {
                    let attempts = Arc::clone(&attempts);
                    let idempotency_keys = Arc::clone(&idempotency_keys);
                    async move {
                        idempotency_keys.lock().unwrap().push(
                            headers
                                .get("idempotency-key")
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_string(),
                        );
                        if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                            (StatusCode::TOO_MANY_REQUESTS, [(header::RETRY_AFTER, "0")])
                                .into_response()
                        } else {
                            (
                                StatusCode::CREATED,
                                [(header::CONTENT_TYPE, "application/vnd.api+json")],
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
    }

    #[tokio::test]
    async fn adds_playlist_tracks_at_the_end_of_the_tidal_playlist() {
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
                            [(header::CONTENT_TYPE, "application/vnd.api+json")],
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
                "meta": { "positionBefore": "end" },
            })]
        );
    }
}
