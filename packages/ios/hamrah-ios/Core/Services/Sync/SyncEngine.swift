import Combine
import Foundation
import Network
import OSLog
import SwiftData

// MARK: - SyncEngine

/// Coordinates outbound/inbound sync, archive prefetch, and cache management.
/// Triggers: app launch/foreground, NWPathMonitor, BGProcessingTask, pull-to-refresh.
final class SyncEngine: ObservableObject {
    let objectWillChange = ObservableObjectPublisher()

    // MARK: - Dependencies

    private let api: LinkAPI
    private let modelContainer: ModelContainer

    private let logger = Logger(subsystem: "app.hamrah.ios", category: "SyncEngine")

    // MARK: - Concurrency

    private let syncQueue = DispatchQueue(label: "SyncEngine.syncQueue", qos: .utility)
    private let pathMonitor = NWPathMonitor()
    private var isSyncing = false

    // MARK: - Init

    /// Default initializer that creates/uses a ModelContainer stored in the App Group.
    convenience init() {
        // Use unified AppModelSchema to guarantee schema consistency across targets
        #if DEBUG
            let container = AppModelSchema.makeSharedContainerWithRecovery()
        #else
            let container =
                (try? AppModelSchema.makeSharedContainer())
                ?? AppModelSchema.makeInMemoryContainer()
        #endif
        self.init(
            api: SecureAPILinkClient(),
            modelContainer: container
        )
    }

    /// Designated initializer supporting dependency injection for testing.
    init(api: LinkAPI, modelContainer: ModelContainer) {
        self.api = api
        self.modelContainer = modelContainer

        setupPathMonitor()
    }

    deinit {
        pathMonitor.cancel()
    }

    // MARK: - Public triggers

    /// Call on app launch, foreground, or pull-to-refresh.
    func triggerSync(reason: String = "manual") {
        syncQueue.async { [weak self] in
            guard let self else { return }
            Task { await self.performSync(reason: reason) }
        }
    }

    /// Immediately runs a full sync on the current task; awaits completion.
    func runSyncNow(reason: String = "manual") async {
        await performSync(reason: reason)
    }

    /// Call from BGProcessingTask (background fetch).
    func triggerBackgroundSync() {
        syncQueue.async { [weak self] in
            guard let self else { return }
            Task { await self.performSync(reason: "background") }
        }
    }

    // MARK: - Core

    private func setupPathMonitor() {
        pathMonitor.pathUpdateHandler = { [weak self] path in
            guard let self else { return }
            if path.status == .satisfied {
                self.logger.debug("NWPathMonitor satisfied → trigger sync.")
                self.triggerSync(reason: "network")
            }
        }
        let queue = DispatchQueue(label: "SyncEngine.pathMonitor")
        pathMonitor.start(queue: queue)
    }

    @MainActor
    private func performSync(reason: String) async {
        guard !isSyncing else { return }
        isSyncing = true
        defer { isSyncing = false }

        let context = modelContainer.mainContext
        logger.info("Starting sync; reason=\(reason, privacy: .public)")

        await syncOutboundLinks(context: context)
        await syncInboundLinks(context: context)

        logger.info("Finished sync; reason=\(reason, privacy: .public)")
    }

    // MARK: - Outbound

    private func queuedLinks(_ context: ModelContext) -> [LinkEntity] {
        (try? context.fetch(
            FetchDescriptor<LinkEntity>(predicate: #Predicate { $0.status == "queued" })
        )) ?? []
    }

    private func payload(for link: LinkEntity, prefs: UserPrefs?) -> OutboundLinkPayload {
        OutboundLinkPayload(
            clientId: link.localId.uuidString,
            url: link.originalUrl.absoluteString,
            sharedText: link.sharedText,
            sourceApp: link.sourceApp,
            sharedAtISO8601: iso8601(link.sharedAt)
        )
    }

    private func updateSynced(_ link: LinkEntity, response: PostLinkResponse) {
        link.serverId = response.serverId
        if let canon = response.canonicalUrl, let canonURL = URL(string: canon) {
            link.canonicalUrl = canonURL
        }
        link.status = "synced"
        link.updatedAt = Date()
        link.lastError = nil
    }

    private func markFailure(_ link: LinkEntity, error: Error) {
        link.attempts += 1
        link.lastError = error.localizedDescription
        if link.attempts > 5 { link.status = "failed" }
        link.updatedAt = Date()
    }

    private func accessToken() -> String? {
        KeychainManager.shared.retrieveString(for: "hamrah_access_token")
    }

    private func currentPrefs(_ context: ModelContext) -> UserPrefs? {
        (try? context.fetch(FetchDescriptor<UserPrefs>()))?.first
    }

    private func syncOutboundLinks(context: ModelContext) async {
        let prefs = currentPrefs(context)
        let token = accessToken()  // may be nil; API layer should handle 401 appropriately

        for link in queuedLinks(context) {
            do {
                let resp = try await api.postLink(
                    payload: payload(for: link, prefs: prefs),
                    token: token
                )
                updateSynced(link, response: resp)
            } catch {
                logger.warning(
                    "POST /links failed: \(error.localizedDescription, privacy: .public)")
                markFailure(link, error: error)
            }
        }

        do { try context.save() } catch {
            logger.error(
                "Failed to save outbound changes: \(error.localizedDescription, privacy: .public)")
        }
    }

    // MARK: - Inbound

    private func syncInboundLinks(context: ModelContext) async {
        let cursor = SyncCursor.fetchOrCreateSingleton(in: context)
        let since = cursor.lastUpdatedCursor ?? ""
        let token = accessToken()

        do {
            let delta = try await api.getLinks(since: since, limit: 100, token: token)
            for serverLink in delta.links {
                mergeServerLink(serverLink, context: context)
            }
            cursor.lastUpdatedCursor = delta.nextCursor
            cursor.lastFullSyncAt = Date()
            try context.save()
        } catch {
            logger.warning(
                "GET /links delta failed: \(error.localizedDescription, privacy: .public)")
        }
    }

    private func mergeServerLink(_ serverLink: ServerLink, context: ModelContext) {
        // Prefer serverId match; otherwise, match by canonicalUrl.
        let links: [LinkEntity]
        if let sid = serverLink.serverId {
            links =
                (try? context.fetch(
                    FetchDescriptor<LinkEntity>(predicate: #Predicate { $0.serverId == sid })
                )) ?? []
        } else if let canonURL = URL(string: serverLink.canonicalUrl) {
            links =
                (try? context.fetch(
                    FetchDescriptor<LinkEntity>(
                        predicate: #Predicate { $0.canonicalUrl == canonURL })
                )) ?? []
        } else {
            return
        }

        let link =
            links.first
            ?? LinkEntity(
                originalUrl: URL(string: serverLink.originalUrl) ?? URL(
                    string: "https://invalid.local")!,
                canonicalUrl: URL(string: serverLink.canonicalUrl) ?? URL(
                    string: "https://invalid.local")!,
                sharedAt: serverLink.sharedAt,
                status: "synced",
                updatedAt: Date(),
                createdAt: serverLink.createdAt
            )

        // Update fields
        link.title = serverLink.title
        link.snippet = serverLink.snippet
        link.summaryShort = serverLink.summaryShort
        link.summaryLong = serverLink.summaryLong
        link.lang = serverLink.lang
        link.saveCount = serverLink.saveCount
        link.status = serverLink.status
        link.updatedAt = Date()
        link.serverId = serverLink.serverId

        // Replace canonicalUrl if server canonicalized it
        if let canonURL = URL(string: serverLink.canonicalUrl) {
            link.canonicalUrl = canonURL
        }

        // Merge tags by name (reuse if already present to avoid duplicates)
        link.tags = mergeTags(names: serverLink.tags, in: context)

        if links.isEmpty { context.insert(link) }
    }

    private func mergeTags(names: [String], in context: ModelContext) -> [TagEntity] {
        guard !names.isEmpty else { return [] }
        var result: [TagEntity] = []

        for name in names {
            let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { continue }
            if let existing = try? context.fetch(
                FetchDescriptor<TagEntity>(predicate: #Predicate { $0.name == trimmed })
            ).first {
                result.append(existing)
            } else {
                let tag = TagEntity(name: trimmed)
                context.insert(tag)
                result.append(tag)
            }
        }
        return result
    }

    // MARK: - Helpers

    private func iso8601(_ date: Date) -> String {
        let f = ISO8601DateFormatter()
        return f.string(from: date)
    }
    #if DEBUG
        /// Test-only convenience to run a full sync immediately.
        /// Intended for unit tests to call without exposing internal methods.
        func _testRunSyncNow(reason: String = "test") async {
            await performSync(reason: reason)
        }
    #endif
}

// MARK: - SwiftData Singleton helpers

extension SyncCursor {
    fileprivate static func fetchOrCreateSingleton(in context: ModelContext) -> SyncCursor {
        if let existing = try? context.fetch(FetchDescriptor<SyncCursor>()).first {
            return existing
        }
        let cursor = SyncCursor()
        context.insert(cursor)
        return cursor
    }
}

// MARK: - Protocols and DTOs

/// Abstracts server operations used by sync so it can be mocked in tests.
protocol LinkAPI {
    func postLink(payload: OutboundLinkPayload, token: String?) async throws -> PostLinkResponse
    func getLinks(since: String, limit: Int, token: String?) async throws -> DeltaResponse
}

// MARK: - LinkAPI DTOs

/// Outbound payload for creating a link (strict schema, snake_case keys):
/// { "url": "...", "client_id": "...", "source_app": "...?", "shared_text": "...?", "shared_at": "...?" }
struct OutboundLinkPayload: Encodable {
    let clientId: String
    let url: String
    let sharedText: String?
    let sourceApp: String?
    let sharedAtISO8601: String

    enum CodingKeys: String, CodingKey {
        case clientId = "client_id"
        case url
        case sharedText = "shared_text"
        case sourceApp = "source_app"
        case sharedAtISO8601 = "shared_at"
    }
}

struct PostLinkResponse: Codable {
    let serverId: String
    let canonicalUrl: String?

    enum CodingKeys: String, CodingKey {
        case serverId = "id"
        case canonicalUrl = "canonical_url"
    }
}

struct DeltaResponse: Codable {
    let links: [ServerLink]
    let nextCursor: String?

    enum CodingKeys: String, CodingKey {
        case links
        case nextCursor = "next_cursor"
    }
}

struct ServerLink: Codable {
    let serverId: String?
    let originalUrl: String
    let canonicalUrl: String
    let title: String?
    let snippet: String?
    let summaryShort: String?
    let summaryLong: String?
    let lang: String?
    let tags: [String]
    let saveCount: Int
    let status: String
    let sharedAt: Date
    let createdAt: Date

    enum CodingKeys: String, CodingKey {
        case serverId = "id"
        case originalUrl = "original_url"
        case canonicalUrl = "canonical_url"
        case title
        case snippet
        case summaryShort = "summary_short"
        case summaryLong = "summary_long"
        case lang
        case tags
        case saveCount = "save_count"
        case status
        case sharedAt = "shared_at"
        case createdAt = "created_at"
    }
}

// MARK: - Concrete API client using SecureAPIService

/// Concrete implementation backed by SecureAPIService and App Attestation.
/// Endpoints follow the backend contract; canonicalization is performed on the server.
final class SecureAPILinkClient: LinkAPI {

    /// POST /v1/links - strict payload (snake_case): url, client_id, source_app?, shared_text?, shared_at? -> returns { id, canonical_url }
    func postLink(payload: OutboundLinkPayload, token: String?) async throws -> PostLinkResponse {
        let body: [String: Any] = try encodeToJSONObject(payload)

        return try await SecureAPIService.shared.post(
            endpoint: "/v1/links",
            body: body,
            accessToken: token,
            responseType: PostLinkResponse.self
        )
    }

    func getLinks(since: String, limit: Int, token: String?) async throws -> DeltaResponse {
        var comps = URLComponents()
        comps.path = "/v1/links"
        comps.queryItems = [
            URLQueryItem(name: "since", value: since),
            URLQueryItem(name: "limit", value: "\(limit)"),
        ]
        let endpoint = comps.string ?? "/v1/links"
        return try await SecureAPIService.shared.get(
            endpoint: endpoint,
            accessToken: token,
            responseType: DeltaResponse.self
        )
    }

    // Encodes Encodable into JSON object dictionary using JSONEncoder then JSONSerialization
    private func encodeToJSONObject<T: Encodable>(_ value: T) throws -> [String: Any] {
        let data = try JSONEncoder.iso8601.encode(value)
        let obj = try JSONSerialization.jsonObject(with: data, options: [])
        guard let dict = obj as? [String: Any] else {
            throw NSError(
                domain: "SecureAPILinkClient", code: 0,
                userInfo: [
                    NSLocalizedDescriptionKey: "Invalid JSON structure"
                ])
        }
        return dict
    }
}

// MARK: - Adapters

extension JSONEncoder {
    static var iso8601: JSONEncoder {
        let enc = JSONEncoder()
        enc.dateEncodingStrategy = .iso8601
        return enc
    }
}
