import Foundation

struct UserPrefsAPI {
    private let client: HamrahAPIClient

    init(client: HamrahAPIClient = .shared) {
        self.client = client
    }

    func load() async throws -> UserPrefsDTO {
        try await client.get(
            "/v1/user/prefs",
            auth: .required,
            responseType: UserPrefsDTO.self
        )
    }

    func save(_ prefs: UserPrefsDTO) async throws -> UserPrefsDTO {
        try await client.put(
            "/v1/user/prefs",
            body: prefs,
            auth: .required,
            responseType: UserPrefsDTO.self
        )
    }
}

struct ModelCatalogAPI {
    private let client: HamrahAPIClient

    init(client: HamrahAPIClient = .shared) {
        self.client = client
    }

    func fetch() async throws -> [AIModelDTO] {
        struct CatalogResponse: Codable { let models: [AIModelDTO] }
        let response: CatalogResponse = try await client.get(
            "/v1/models",
            auth: .optional,
            responseType: CatalogResponse.self
        )
        return response.models
    }
}

struct PasskeyAPI {
    private let client: HamrahAPIClient

    init(client: HamrahAPIClient = .shared) {
        self.client = client
    }

    func list(userId: String) async throws -> [PasskeyCredential] {
        let response: PasskeyListResponse = try await client.get(
            "/api/webauthn/users/\(Self.pathComponent(userId))/credentials",
            auth: .required,
            responseType: PasskeyListResponse.self
        )
        return response.credentials
    }

    func delete(credentialId: String) async throws {
        let _: APIResponse = try await client.delete(
            "/api/webauthn/credentials/\(Self.pathComponent(credentialId))",
            auth: .required,
            responseType: APIResponse.self
        )
    }

    private static func pathComponent(_ value: String) -> String {
        value.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? value
    }
}
