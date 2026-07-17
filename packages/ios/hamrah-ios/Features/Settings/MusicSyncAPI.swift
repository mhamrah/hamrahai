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
    let total_items: Int
    let imported_items: Int
    let unmatched_items: Int
    let error: String?
    let created_at: String
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
}
