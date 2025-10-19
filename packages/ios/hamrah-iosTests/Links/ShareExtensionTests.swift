import Foundation
import XCTest

@testable import hamrah_ios

final class ShareExtensionTests: XCTestCase {

    // MARK: - In-memory store and upsert logic (model-level only)

    final class InMemoryStore {
        var links: [LinkEntity] = []

        @discardableResult
        func upsert(
            url: URL,
            title: String?,
            sharedText: String?,
            sourceApp: String?,
            now: Date = Date()
        ) -> LinkEntity {
            // Server-side canonicalization: initialize canonicalUrl with original URL
            let canonicalURL = url

            // Dedupe by originalUrl first; fallback to canonicalUrl
            if let existing = links.first(where: { $0.originalUrl == url })
                ?? links.first(where: { $0.canonicalUrl == canonicalURL })
            {
                existing.saveCount += 1
                existing.lastSavedAt = now
                if existing.title == nil, let title = title { existing.title = title }
                if existing.snippet == nil, let txt = sharedText { existing.snippet = txt }
                existing.updatedAt = now
                return existing
            } else {
                // Insert new LinkEntity
                let link = LinkEntity(
                    originalUrl: url,
                    canonicalUrl: canonicalURL,
                    title: title,
                    snippet: sharedText,
                    sourceApp: sourceApp,
                    sharedText: sharedText,
                    sharedAt: now,
                    status: "queued",
                    updatedAt: now,
                    createdAt: now
                )
                links.append(link)
                return link
            }
        }
    }

    // MARK: - Tests

    func testUpsert_insertsNewLink() {
        let store = InMemoryStore()
        let url = URL(string: "https://example.com/foo")!
        let now = Date()

        let link = store.upsert(
            url: url,
            title: "Title",
            sharedText: "Snippet",
            sourceApp: "Safari",
            now: now
        )

        XCTAssertEqual(store.links.count, 1)
        XCTAssertEqual(link.originalUrl, url)
        XCTAssertEqual(link.canonicalUrl, url)
        XCTAssertEqual(link.status, "queued")
        XCTAssertEqual(link.saveCount, 1)
        XCTAssertEqual(link.title, "Title")
        XCTAssertEqual(link.snippet, "Snippet")
        XCTAssertEqual(link.createdAt, now)
        XCTAssertEqual(link.updatedAt, now)
    }

    func testUpsert_dedupeIncrementsSaveCountAndUpdatesLastSavedAt() {
        let store = InMemoryStore()
        let url = URL(string: "https://example.com/foo")!

        let earlier = Date().addingTimeInterval(-3600)
        _ = store.upsert(
            url: url,
            title: nil,
            sharedText: nil,
            sourceApp: "Safari",
            now: earlier
        )

        let now = Date()
        let link = store.upsert(
            url: url,
            title: "New Title",
            sharedText: "New Snippet",
            sourceApp: "Safari",
            now: now
        )

        XCTAssertEqual(store.links.count, 1, "Should not create a new link on dedupe")
        XCTAssertEqual(link.saveCount, 2)
        XCTAssertEqual(link.lastSavedAt, now)
        XCTAssertEqual(link.updatedAt, now)
        // Title/snippet were nil; should be patched in
        XCTAssertEqual(link.title, "New Title")
        XCTAssertEqual(link.snippet, "New Snippet")
    }

    func testUpsert_doesNotChangeStatusOrOverwriteExistingTitle() {
        let store = InMemoryStore()
        let url = URL(string: "https://example.com/foo")!
        let t0 = Date().addingTimeInterval(-7200)

        // First insert
        var link = store.upsert(
            url: url,
            title: "Original Title",
            sharedText: "Snippet",
            sourceApp: "Safari",
            now: t0
        )
        // Simulate it being synced already
        link.status = "synced"

        // Second upsert (duplicate) with a different title; should not override
        let t1 = Date()
        link = store.upsert(
            url: url,
            title: "Different Title",
            sharedText: "Other Snippet",
            sourceApp: "Safari",
            now: t1
        )

        XCTAssertEqual(link.status, "synced", "Dedupe upsert should not change status")
        XCTAssertEqual(link.title, "Original Title", "Existing title should not be overwritten")
        XCTAssertEqual(link.saveCount, 2)
        XCTAssertEqual(link.lastSavedAt, t1)
        XCTAssertEqual(link.updatedAt, t1)
    }
}
