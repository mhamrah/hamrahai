import Foundation

struct MusicConnectionDTO: Codable, Identifiable {
    let provider: String
    let provider_account_id: String?
    let provider_account_name: String?
    let status: String
    let connected_at: String?
    let last_error: String?

    var id: String { provider }
}

struct MusicImportRequestDTO: Codable, Equatable {
    let include_owned_playlists: Bool
    let include_saved_playlists: Bool
    let include_followed_artists: Bool
    let include_saved_tracks: Bool
}

struct MusicImportDTO: Codable, Identifiable {
    let id: String
    let status: String
    let include_owned_playlists: Bool
    let include_saved_playlists: Bool
    let include_followed_artists: Bool
    let include_saved_tracks: Bool
    let stage: String
    let total_items: Int
    let imported_items: Int
    let unmatched_items: Int
    let playlist_total: Int
    let playlists_imported: Int
    let artist_total: Int
    let artists_checked: Int
    let artists_matched: Int
    let artists_followed: Int
    let playlist_track_total: Int
    let playlist_tracks_imported: Int
    let saved_track_total: Int
    let saved_tracks_imported: Int
    let tracks_matched: Int
    let error: String?
    let created_at: String

    var isActive: Bool { status == "queued" || status == "running" }
    var canRestart: Bool { status == "failed" || status == "partial" }
    var importOptionsAreEditable: Bool { !isActive }

    var sourceSummary: String {
        let playlists = "\(playlist_total) Spotify \(playlist_total == 1 ? "playlist" : "playlists")"
        let tracks = "\(playlist_track_total) playlist \(playlist_track_total == 1 ? "track" : "tracks")"
        let likedTracks = "\(saved_track_total) Liked \(saved_track_total == 1 ? "Song" : "Songs")"
        let artists = "\(artist_total) followed \(artist_total == 1 ? "artist" : "artists")"
        return "Selected from Spotify: \(playlists), \(tracks), \(likedTracks), and \(artists)."
    }

    var stageDescription: String {
        if status == "failed" {
            return switch stage {
            case "reading_spotify": "Import failed while reading Spotify"
            case "creating_playlists": "Import failed while creating TIDAL playlists"
            case "adding_playlist_tracks": "Import failed while matching or adding playlist tracks"
            case "matching_artists", "following_artists": "Import failed while matching or following artists"
            case "saving_liked_tracks": "Import failed while saving Liked Songs"
            default: "Import failed"
            }
        }
        return switch stage {
        case "queued", "preparing": "Preparing secure connections"
        case "reading_spotify": "Reading selected Spotify collections"
        case "creating_playlists": "Creating TIDAL playlists"
        case "adding_playlist_tracks": "Adding exact playlist-track matches to TIDAL"
        case "matching_artists": "Finding exact artist matches in TIDAL"
        case "following_artists": "Following exact artist matches in TIDAL"
        case "saving_liked_tracks": "Saving exact Liked Song matches to TIDAL"
        case "completed": status == "partial" ? "Completed with unmatched items" : "Completed"
        case "failed": "Import failed"
        default: stage.replacingOccurrences(of: "_", with: " ").capitalized
        }
    }

    var stageProgress: (current: Int, total: Int, label: String)? {
        switch stage {
        case "creating_playlists" where playlist_total > 0:
            return (playlists_imported, playlist_total, "\(playlists_imported) of \(playlist_total) playlists created")
        case "adding_playlist_tracks" where playlist_track_total > 0:
            return (playlist_tracks_imported, playlist_track_total, "\(playlist_tracks_imported) of \(playlist_track_total) playlist tracks added")
        case "matching_artists" where artist_total > 0:
            return (artists_checked, artist_total, "\(artists_checked) of \(artist_total) artists checked")
        case "following_artists" where artists_matched > 0:
            return (artists_followed, artists_matched, "\(artists_followed) of \(artists_matched) exact matches followed")
        case "saving_liked_tracks" where saved_track_total > 0:
            return (saved_tracks_imported, saved_track_total, "\(saved_tracks_imported) of \(saved_track_total) Liked Songs saved")
        default:
            return nil
        }
    }

    var resultSummary: String {
        "\(playlists_imported) playlists created · \(playlist_tracks_imported) playlist tracks added · \(saved_tracks_imported) Liked Songs saved · \(artists_followed) artist follows completed · \(unmatched_items) unmatched"
    }

    var transferSummary: String {
        "\(imported_items) of \(total_items) selected items transferred"
    }

    var recoveryMessage: String? {
        if let error { return error }
        return canRestart
            ? "Restart the incomplete import to safely reuse its original TIDAL idempotency keys."
            : nil
    }

    var shortReference: String { String(id.prefix(8)) }

    var createdAtDescription: String {
        guard let date = ISO8601DateFormatter().date(from: created_at) else { return created_at }
        return date.formatted(date: .abbreviated, time: .shortened)
    }
}

struct MusicUnmatchedTrackDTO: Codable, Identifiable {
    let id: String
    let source_collection: String
    let track_name: String
    let artist_name: String?
    let album_name: String?
    let isrc: String?
    let reason: String

    var detail: String {
        [artist_name, album_name].compactMap { $0 }.joined(separator: " · ")
    }

    var reasonDescription: String {
        if reason == "missing_isrc" { return "The service did not provide an ISRC" }
        return reason == "not_available_in_spotify" ? "No exact ISRC match was found in Spotify" : "No exact ISRC match was found in TIDAL"
    }
}

struct MusicAuthorizationDTO: Codable { let authorization_url: String }

struct MusicSyncAPI {
    private let client: HamrahAPIClient

    init(client: HamrahAPIClient = .shared) { self.client = client }

    func connections() async throws -> [MusicConnectionDTO] {
        try await client.get("/v1/music/connections", auth: .required, responseType: [MusicConnectionDTO].self)
    }

    func beginConnection(provider: String) async throws -> URL {
        let result: MusicAuthorizationDTO = try await client.post(
            "/v1/music/connections/\(provider)/authorize",
            body: ["redirect_path": "/settings"],
            auth: .required,
            responseType: MusicAuthorizationDTO.self)
        guard let url = URL(string: result.authorization_url) else { throw URLError(.badURL) }
        return url
    }

    func disconnectConnection(provider: String) async throws {
        let _: EmptyResponse = try await client.delete(
            "/v1/music/connections/\(provider)",
            auth: .required,
            responseType: EmptyResponse.self)
    }

    func startImport(
        includeSavedPlaylists: Bool,
        includeSavedTracks: Bool
    ) async throws -> MusicImportDTO {
        try await client.post(
            "/v1/music/imports",
            body: MusicImportRequestDTO(
                include_owned_playlists: true,
                include_saved_playlists: includeSavedPlaylists,
                include_followed_artists: true,
                include_saved_tracks: includeSavedTracks),
            auth: .required,
            responseType: MusicImportDTO.self)
    }

    func restartImport(
        id: String,
        includeSavedPlaylists: Bool,
        includeSavedTracks: Bool
    ) async throws -> MusicImportDTO {
        try await client.post(
            "/v1/music/imports/\(id)/restart",
            body: MusicImportRequestDTO(
                include_owned_playlists: true,
                include_saved_playlists: includeSavedPlaylists,
                include_followed_artists: true,
                include_saved_tracks: includeSavedTracks),
            auth: .required,
            responseType: MusicImportDTO.self)
    }

    func imports() async throws -> [MusicImportDTO] {
        try await client.get("/v1/music/imports", auth: .required, responseType: [MusicImportDTO].self)
    }

    func unmatchedTracks(importID: String) async throws -> [MusicUnmatchedTrackDTO] {
        try await client.get("/v1/music/imports/\(importID)/unmatched-tracks", auth: .required, responseType: [MusicUnmatchedTrackDTO].self)
    }
}
