import SwiftData
import XCTest

@testable import hamrah_ios

final class SyncEngineTests: XCTestCase {

    // MARK: - Mocks

    final class MockAPI: LinkAPI {
        var capturedPostPayloads: [OutboundLinkPayload] = []
        var nextPostResult: Result<PostLinkResponse, Error> = .success(
            PostLinkResponse(serverId: "srv-1", canonicalUrl: "https://example.com/canon")
        )

        var nextDeltaResult: Result<DeltaResponse, Error> = .success(
            DeltaResponse(
                links: [],
                nextCursor: "cursor-next"
            )
        )

        func postLink(payload: OutboundLinkPayload, token: String?) async throws -> PostLinkResponse
        {
            capturedPostPayloads.append(payload)
            return try nextPostResult.get()
        }

        func getLinks(since: String, limit: Int, token: String?) async throws -> DeltaResponse {
            return try nextDeltaResult.get()
        }
    }

    enum TestError: Error {
        case network
    }

    // MARK: - Helpers

    private func makeInMemoryContainer() throws -> ModelContainer {
        let config = ModelConfiguration(isStoredInMemoryOnly: true)
        return try ModelContainer(
            for:
                LinkEntity.self,

            TagEntity.self,
            SyncCursor.self,
            UserPrefs.self,
            configurations: config
        )
    }

    private func fetchAllLinks(_ context: ModelContext) -> [LinkEntity] {
        (try? context.fetch(FetchDescriptor<LinkEntity>())) ?? []
    }

    private func fetchAllTags(_ context: ModelContext) -> [TagEntity] {
        (try? context.fetch(FetchDescriptor<TagEntity>())) ?? []
    }

    // MARK: - Tests

    func testOutboundSync_postsQueuedLinks_andUpdatesStatusAndCanonicalURL() async throws {
        let container = try makeInMemoryContainer()
        let context = container.mainContext

        // Insert a queued link
        let original = URL(string: "https://example.com/path")!
        let link = LinkEntity(
            originalUrl: original,
            canonicalUrl: original,  // server will update canonical
            sharedAt: Date(),
            status: "queued",
            updatedAt: Date(),
            createdAt: Date()
        )
        context.insert(link)
        try context.save()

        // Mock API success response
        let api = MockAPI()
        api.nextPostResult = .success(
            PostLinkResponse(serverId: "server-123", canonicalUrl: "https://example.com/canonical")
        )

        let engine = SyncEngine(api: api, modelContainer: container)

        await engine._testRunSyncNow(reason: "test")

        // Assert
        let stored = fetchAllLinks(context)
        XCTAssertEqual(stored.count, 1)
        let saved = try XCTUnwrap(stored.first)
        XCTAssertEqual(saved.status, "synced")
        XCTAssertEqual(saved.serverId, "server-123")
        XCTAssertEqual(saved.canonicalUrl.absoluteString, "https://example.com/canonical")

        XCTAssertEqual(api.capturedPostPayloads.count, 1)
        let payload = try XCTUnwrap(api.capturedPostPayloads.first)
        XCTAssertEqual(payload.originalUrl, "https://example.com/path")
    }

    func testOutboundSync_failureIncrementsAttemptsAndLeavesQueued() async throws {
        let container = try makeInMemoryContainer()
        let context = container.mainContext

        // Insert a queued link
        let url = URL(string: "https://example.com/fail")!
        let link = LinkEntity(
            originalUrl: url,
            canonicalUrl: url,
            sharedAt: Date(),
            status: "queued",
            updatedAt: Date(),
            createdAt: Date()
        )
        context.insert(link)
        try context.save()

        // Mock API failure
        let api = MockAPI()
        api.nextPostResult = .failure(TestError.network)

        let engine = SyncEngine(
            api: api, modelContainer: container)

        await engine._testRunSyncNow(reason: "test")

        // Assert
        let stored = fetchAllLinks(context)
        let saved = try XCTUnwrap(stored.first)
        XCTAssertEqual(saved.status, "queued", "Should remain queued after failure")
        XCTAssertEqual(saved.attempts, 1, "Attempts should increment on failure")
        XCTAssertNotNil(saved.lastError, "Last error should be recorded")
    }

    func testInboundSync_mergesServerLinks_andUpdatesCursor() async throws {
        let container = try makeInMemoryContainer()
        let context = container.mainContext

        // No existing link; inbound delta should create
        let now = Date()
        let serverLink = ServerLink(
            serverId: "server-789",
            originalUrl: "https://example.com/original",
            canonicalUrl: "https://example.com/canon",
            title: "Title",
            snippet: "Snippet",
            summaryShort: "Short",
            summaryLong: "Long",
            lang: "en",
            tags: ["swift", "ios"],
            saveCount: 3,
            status: "synced",
            sharedAt: now,
            createdAt: now.addingTimeInterval(-3600)
        )

        let api = MockAPI()
        api.nextDeltaResult = .success(DeltaResponse(links: [serverLink], nextCursor: "cursor-2"))

        let engine = SyncEngine(
            api: api, modelContainer: container)

        await engine._testRunSyncNow(reason: "test")

        // Assert link created and fields merged
        let links = fetchAllLinks(context)
        XCTAssertEqual(links.count, 1)
        let saved = try XCTUnwrap(links.first)
        XCTAssertEqual(saved.serverId, "server-789")
        XCTAssertEqual(saved.title, "Title")
        XCTAssertEqual(saved.snippet, "Snippet")
        XCTAssertEqual(saved.summaryShort, "Short")
        XCTAssertEqual(saved.summaryLong, "Long")
        XCTAssertEqual(saved.lang, "en")
        XCTAssertEqual(saved.saveCount, 3)
        XCTAssertEqual(saved.canonicalUrl.absoluteString, "https://example.com/canon")

        // Assert tags merged without duplicates
        XCTAssertEqual(saved.tags.map(\.name).sorted(), ["ios", "swift"])
        XCTAssertEqual(fetchAllTags(context).map(\.name).sorted(), ["ios", "swift"])

        // Assert cursor updated
        let cursor = try XCTUnwrap((try? context.fetch(FetchDescriptor<SyncCursor>()))?.first)
        XCTAssertEqual(cursor.lastUpdatedCursor, "cursor-2")
        XCTAssertNotNil(cursor.lastFullSyncAt)
    }

    func testInboundSync_mergesIntoExistingByServerId() async throws {
        let container = try makeInMemoryContainer()
        let context = container.mainContext

        // Existing link with serverId
        let existing = LinkEntity(
            originalUrl: URL(string: "https://old.example.com")!,
            canonicalUrl: URL(string: "https://old.example.com")!,
            sharedAt: Date(),
            status: "queued",
            updatedAt: Date(),
            createdAt: Date()
        )
        existing.serverId = "server-111"
        context.insert(existing)
        try context.save()

        // Delta with same serverId should merge into existing record
        let s = ServerLink(
            serverId: "server-111",
            originalUrl: "https://new.example.com/original",
            canonicalUrl: "https://new.example.com/canon",
            title: "New Title",
            snippet: "New Snippet",
            summaryShort: nil,
            summaryLong: nil,
            lang: nil,
            tags: ["merge"],
            saveCount: 5,
            status: "synced",
            sharedAt: Date(),
            createdAt: Date().addingTimeInterval(-1000)
        )

        let api = MockAPI()
        api.nextDeltaResult = .success(DeltaResponse(links: [s], nextCursor: nil))

        let engine = SyncEngine(
            api: api, modelContainer: container)
        await engine._testRunSyncNow(reason: "test")

        // Assert merged
        let links = fetchAllLinks(context)
        XCTAssertEqual(links.count, 1)
        let saved = try XCTUnwrap(links.first)
        XCTAssertEqual(saved.serverId, "server-111")
        XCTAssertEqual(saved.title!, "New Title")
        XCTAssertEqual(saved.snippet!, "New Snippet")
        XCTAssertEqual(saved.canonicalUrl.absoluteString, "https://new.example.com/canon")
        XCTAssertEqual(saved.saveCount, 5)
        XCTAssertEqual(saved.status, "synced")
        XCTAssertEqual(saved.tags.map(\.name), ["merge"])
    }
}
