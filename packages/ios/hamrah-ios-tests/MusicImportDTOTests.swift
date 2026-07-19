import Foundation
import Testing
@testable import hamrah_ios

struct MusicImportDTOTests {
    @Test
    func matchingStageExplainsSourceAndChecks() {
        let musicImport = makeImport(
            stage: "matching_artists",
            playlistTotal: 3,
            artistTotal: 12,
            artistsChecked: 7
        )

        #expect(musicImport.isActive)
        #expect(musicImport.sourceSummary == "Selected from Spotify: 3 Spotify playlists, 0 playlist tracks, 0 Liked Songs, and 12 followed artists.")
        #expect(musicImport.stageDescription == "Finding exact artist matches in TIDAL")
        #expect(musicImport.stageProgress?.current == 7)
        #expect(musicImport.stageProgress?.total == 12)
        #expect(musicImport.stageProgress?.label == "7 of 12 artists checked")
    }

    @Test
    func followingStageShowsExactMatchProgress() {
        let musicImport = makeImport(
            stage: "following_artists",
            artistsMatched: 8,
            artistsFollowed: 5
        )

        #expect(musicImport.stageProgress?.current == 5)
        #expect(musicImport.stageProgress?.total == 8)
        #expect(musicImport.stageProgress?.label == "5 of 8 exact matches followed")
    }

    @Test
    func playlistTrackStageShowsCopiedTrackProgress() {
        let musicImport = makeImport(
            stage: "adding_playlist_tracks",
            playlistTrackTotal: 11,
            playlistTracksImported: 7
        )

        #expect(musicImport.stageDescription == "Adding exact playlist-track matches to TIDAL")
        #expect(musicImport.stageProgress?.current == 7)
        #expect(musicImport.stageProgress?.total == 11)
        #expect(musicImport.stageProgress?.label == "7 of 11 playlist tracks added")
    }

    @Test
    func reconciliationStageAndScheduledActivityRemainVisible() {
        let reconciling = makeImport(
            stage: "reconciling_tidal_playlists",
            activity: "Consolidating TIDAL playlist content: Favorites"
        )
        let scheduled = makeImport(
            status: "queued",
            stage: "queued",
            activity: "Waiting for provider rate limit; the import will resume automatically"
        )

        #expect(reconciling.stageDescription == "Reconciling existing TIDAL playlists with Spotify")
        #expect(reconciling.activity == "Consolidating TIDAL playlist content: Favorites")
        #expect(scheduled.isActive)
        #expect(scheduled.activity == "Waiting for provider rate limit; the import will resume automatically")
    }

    @Test
    func transferSummaryShowsTransferredItemsOutOfTheSelectedTotal() {
        let musicImport = makeImport(
            stage: "creating_playlists",
            playlistTotal: 3,
            playlistsImported: 2,
            artistTotal: 12
        )

        #expect(musicImport.transferSummary == "2 of 15 selected items transferred")
    }

    @Test
    func failedAndPartialImportsCanBeRestarted() {
        #expect(makeImport(status: "failed", stage: "failed").canRestart)
        #expect(makeImport(status: "partial", stage: "completed").canRestart)
        #expect(!makeImport(status: "completed", stage: "completed").canRestart)
    }

    @Test
    func importOptionsRemainEditableForFailedAndPartialRetries() {
        #expect(makeImport(status: "failed", stage: "failed").importOptionsAreEditable)
        #expect(makeImport(status: "partial", stage: "completed").importOptionsAreEditable)
        #expect(!makeImport(status: "running", stage: "reading_spotify").importOptionsAreEditable)
    }

    @Test
    @MainActor
    func retryOptionsEncodeUsingTheSharedSnakeCaseContract() throws {
        let options = MusicImportRequestDTO(
            include_owned_playlists: true,
            include_saved_playlists: true,
            include_followed_artists: true,
            include_saved_tracks: true
        )

        let data = try JSONEncoder().encode(options)
        let decoded = try JSONDecoder().decode(MusicImportRequestDTO.self, from: data)

        #expect(decoded == options)
    }

    @Test
    func recoveryMessagePrefersTheServerProvidedReason() {
        let importWithScopeError = makeImport(
            status: "failed",
            stage: "failed",
            error: "Spotify authorization needs user-library-read; reconnect Spotify to continue"
        )

        #expect(importWithScopeError.recoveryMessage == "Spotify authorization needs user-library-read; reconnect Spotify to continue")
        #expect(makeImport(status: "failed", stage: "failed").recoveryMessage == "Restart the incomplete import to safely reuse its original TIDAL idempotency keys.")
    }

    @Test
    func failedImportNamesTheProviderOperationAndReference() {
        let musicImport = makeImport(
            status: "failed",
            stage: "adding_playlist_tracks"
        )

        #expect(musicImport.stageDescription == "Import failed while matching or adding playlist tracks")
        #expect(musicImport.shortReference == "import-1")
    }

    private func makeImport(
        status: String = "running",
        stage: String,
        playlistTotal: Int = 0,
        playlistsImported: Int = 0,
        artistTotal: Int = 0,
        artistsChecked: Int = 0,
        artistsMatched: Int = 0,
        artistsFollowed: Int = 0,
        playlistTrackTotal: Int = 0,
        playlistTracksImported: Int = 0,
        savedTrackTotal: Int = 0,
        savedTracksImported: Int = 0,
        activity: String = "Preparing secure connections",
        error: String? = nil
    ) -> MusicImportDTO {
        MusicImportDTO(
            id: "import-1",
            status: status,
            include_owned_playlists: true,
            include_saved_playlists: false,
            include_followed_artists: true,
            include_saved_tracks: true,
            stage: stage,
            total_items: playlistTotal + artistTotal,
            imported_items: playlistsImported + artistsFollowed,
            unmatched_items: 0,
            playlist_total: playlistTotal,
            playlists_imported: playlistsImported,
            artist_total: artistTotal,
            artists_checked: artistsChecked,
            artists_matched: artistsMatched,
            artists_followed: artistsFollowed,
            playlist_track_total: playlistTrackTotal,
            playlist_tracks_imported: playlistTracksImported,
            saved_track_total: savedTrackTotal,
            saved_tracks_imported: savedTracksImported,
            tracks_matched: playlistTracksImported + savedTracksImported,
            activity: activity,
            error: error,
            created_at: "2026-07-17T00:00:00Z"
        )
    }
}
