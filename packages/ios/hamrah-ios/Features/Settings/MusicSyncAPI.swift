import Foundation

struct MusicConnectionDTO: Codable, Identifiable {
    let provider: String
    let provider_account_id: String?
    let status: String
    let connected_at: String?
    let last_error: String?

    var id: String { provider }
}

struct MusicImportRequestDTO: Codable {
    let include_owned_playlists: Bool
    let include_saved_playlists: Bool
    let include_followed_artists: Bool
}

struct MusicImportDTO: Codable, Identifiable {
    let id: String
    let status: String
    let include_owned_playlists: Bool
    let include_saved_playlists: Bool
    let include_followed_artists: Bool
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
    let error: String?
    let created_at: String

    var isActive: Bool { status == "queued" || status == "running" }

    var sourceSummary: String {
        let playlists = "\(playlist_total) Spotify \(playlist_total == 1 ? "playlist" : "playlists")"
        let artists = "\(artist_total) followed \(artist_total == 1 ? "artist" : "artists")"
        return "Selected from Spotify: \(playlists) and \(artists)."
    }

    var stageDescription: String {
        switch stage {
        case "queued", "preparing": "Preparing secure connections"
        case "reading_spotify": "Reading selected Spotify collections"
        case "creating_playlists": "Creating empty TIDAL playlists"
        case "matching_artists": "Finding exact artist matches in TIDAL"
        case "following_artists": "Following exact artist matches in TIDAL"
        case "completed": status == "partial" ? "Completed with unmatched artists" : "Completed"
        case "failed": "Import failed"
        default: stage.replacingOccurrences(of: "_", with: " ").capitalized
        }
    }

    var stageProgress: (current: Int, total: Int, label: String)? {
        switch stage {
        case "creating_playlists" where playlist_total > 0:
            return (playlists_imported, playlist_total, "\(playlists_imported) of \(playlist_total) playlists created")
        case "matching_artists" where artist_total > 0:
            return (artists_checked, artist_total, "\(artists_checked) of \(artist_total) artists checked")
        case "following_artists" where artists_matched > 0:
            return (artists_followed, artists_matched, "\(artists_followed) of \(artists_matched) exact matches followed")
        default:
            return nil
        }
    }

    var resultSummary: String {
        "\(playlists_imported) playlists created · \(artists_followed) artists followed · \(unmatched_items) unmatched"
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

    func startImport(includeSavedPlaylists: Bool) async throws -> MusicImportDTO {
        try await client.post(
            "/v1/music/imports",
            body: MusicImportRequestDTO(include_owned_playlists: true, include_saved_playlists: includeSavedPlaylists, include_followed_artists: true),
            auth: .required,
            responseType: MusicImportDTO.self)
    }

    func imports() async throws -> [MusicImportDTO] {
        try await client.get("/v1/music/imports", auth: .required, responseType: [MusicImportDTO].self)
    }
}
