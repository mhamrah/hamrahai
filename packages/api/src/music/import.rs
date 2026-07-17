use std::{collections::HashSet, future::Future};

use reqwest::Client;
use serde::Deserialize;
use uuid::Uuid;

use super::query_value;

const SPOTIFY_API_BASE: &str = "https://api.spotify.com";
const TIDAL_API_BASE: &str = "https://openapi.tidal.com/v2";

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
    async fn spotify_followed_artists(&self) -> Result<Vec<SpotifyArtist>, String>;
    async fn create_tidal_playlist(
        &self,
        playlist: &SpotifyPlaylist,
        visibility: TidalPlaylistVisibility,
        idempotency_key: String,
    ) -> Result<(), String>;
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

pub(super) struct HttpMusicImportProvider {
    client: Client,
    spotify_access_token: String,
    tidal_access_token: String,
}

impl HttpMusicImportProvider {
    pub(super) fn new(spotify_access_token: String, tidal_access_token: String) -> Self {
        Self {
            client: Client::new(),
            spotify_access_token,
            tidal_access_token,
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
        self.client
            .get(url)
            .bearer_auth(&self.tidal_access_token)
            .header("accept", "application/vnd.api+json")
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?
            .json()
            .await
            .map_err(|error| error.to_string())
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
    ) -> Result<(), String> {
        let body = serde_json::json!({
            "data": {
                "type": "playlists",
                "attributes": {
                    "name": playlist.name,
                    "accessType": visibility.as_api_value(),
                }
            }
        });
        self.client
            .post(format!("{TIDAL_API_BASE}/playlists"))
            .bearer_auth(&self.tidal_access_token)
            .header("accept", "application/vnd.api+json")
            .header("content-type", "application/vnd.api+json")
            .header("idempotency-key", idempotency_key)
            .json(&body)
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    async fn find_tidal_artist(&self, name: &str) -> Result<Option<String>, String> {
        let response: TidalSearchResponse = self
            .tidal_get(format!(
                "{TIDAL_API_BASE}/searchResults/{}?include=artists",
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
            .client
            .post(format!(
                "{TIDAL_API_BASE}/userCollectionArtists/me/relationships/items"
            ))
            .bearer_auth(&self.tidal_access_token)
            .header("accept", "application/vnd.api+json")
            .header("content-type", "application/vnd.api+json")
            .header("idempotency-key", idempotency_key)
            .json(&body)
            .send()
            .await
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?
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
    outcome.total_items = (playlists.len() + artists.len()) as i32;
    progress.playlist_total = playlists.len() as i32;
    progress.artist_total = artists.len() as i32;
    progress.stage = if playlists.is_empty() {
        "matching_artists"
    } else {
        "creating_playlists"
    };
    report_progress(progress.clone()).await;

    for playlist in playlists {
        let visibility = if playlist.is_public {
            TidalPlaylistVisibility::Public
        } else {
            TidalPlaylistVisibility::Unlisted
        };
        provider
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
    Ok((outcome, progress))
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
struct SpotifyArtistPage {
    items: Vec<SpotifyArtistWire>,
    next: Option<String>,
}

#[derive(Deserialize)]
struct SpotifyArtistWire {
    name: String,
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    struct FakeProvider {
        playlists: Vec<SpotifyPlaylist>,
        artists: Vec<SpotifyArtist>,
        matched_artist: Option<String>,
        created_playlists: Mutex<Vec<(String, TidalPlaylistVisibility)>>,
        followed_artists: Mutex<Vec<String>>,
    }

    impl MusicImportProvider for FakeProvider {
        async fn spotify_current_user_id(&self) -> Result<String, String> {
            Ok("spotify-user".to_string())
        }

        async fn spotify_playlists(&self) -> Result<Vec<SpotifyPlaylist>, String> {
            Ok(self.playlists.clone())
        }

        async fn spotify_followed_artists(&self) -> Result<Vec<SpotifyArtist>, String> {
            Ok(self.artists.clone())
        }

        async fn create_tidal_playlist(
            &self,
            playlist: &SpotifyPlaylist,
            visibility: TidalPlaylistVisibility,
            _idempotency_key: String,
        ) -> Result<(), String> {
            self.created_playlists
                .lock()
                .unwrap()
                .push((playlist.id.clone(), visibility));
            Ok(())
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

    #[tokio::test]
    async fn imports_owned_playlists_with_matching_visibility_and_follows_exact_artist_matches() {
        let provider = FakeProvider {
            playlists: vec![
                playlist("public-owned", "spotify-user", true),
                playlist("private-owned", "spotify-user", false),
                playlist("saved", "other-user", true),
            ],
            artists: vec![SpotifyArtist {
                name: "The Artist".to_string(),
            }],
            matched_artist: Some("tidal-artist".to_string()),
            created_playlists: Mutex::new(Vec::new()),
            followed_artists: Mutex::new(Vec::new()),
        };

        let outcome = execute_import_with_progress(
            &provider,
            Uuid::nil(),
            ImportOptions {
                include_owned_playlists: true,
                include_saved_playlists: false,
                include_followed_artists: true,
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
            artists: Vec::new(),
            matched_artist: None,
            created_playlists: Mutex::new(Vec::new()),
            followed_artists: Mutex::new(Vec::new()),
        };

        let outcome = execute_import_with_progress(
            &provider,
            Uuid::nil(),
            ImportOptions {
                include_owned_playlists: false,
                include_saved_playlists: true,
                include_followed_artists: false,
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
    async fn reports_collection_specific_import_progress() {
        let provider = FakeProvider {
            playlists: vec![playlist("owned", "spotify-user", true)],
            artists: vec![SpotifyArtist {
                name: "The Artist".to_string(),
            }],
            matched_artist: Some("tidal-artist".to_string()),
            created_playlists: Mutex::new(Vec::new()),
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
}
