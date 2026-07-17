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
        #expect(musicImport.sourceSummary == "Selected from Spotify: 3 Spotify playlists and 12 followed artists.")
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
    func failedAndPartialImportsCanBeRestarted() {
        #expect(makeImport(status: "failed", stage: "failed").canRestart)
        #expect(makeImport(status: "partial", stage: "completed").canRestart)
        #expect(!makeImport(status: "completed", stage: "completed").canRestart)
    }

    private func makeImport(
        status: String = "running",
        stage: String,
        playlistTotal: Int = 0,
        artistTotal: Int = 0,
        artistsChecked: Int = 0,
        artistsMatched: Int = 0,
        artistsFollowed: Int = 0
    ) -> MusicImportDTO {
        MusicImportDTO(
            id: "import-1",
            status: status,
            include_owned_playlists: true,
            include_saved_playlists: false,
            include_followed_artists: true,
            stage: stage,
            total_items: playlistTotal + artistTotal,
            imported_items: artistsFollowed,
            unmatched_items: 0,
            playlist_total: playlistTotal,
            playlists_imported: 0,
            artist_total: artistTotal,
            artists_checked: artistsChecked,
            artists_matched: artistsMatched,
            artists_followed: artistsFollowed,
            error: nil,
            created_at: "2026-07-17T00:00:00Z"
        )
    }
}
