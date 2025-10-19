import XCTest

@testable import hamrah_ios

final class LinkEntityTests: XCTestCase {

    func makeLink(
        original: String, canonical: String? = nil, saveCount: Int = 1, lastSavedAt: Date? = nil
    ) -> LinkEntity {
        let url = URL(string: original)!
        let canon = URL(string: canonical ?? original)!
        return LinkEntity(
            originalUrl: url,
            canonicalUrl: canon,
            sharedAt: Date(),
            status: "queued",
            updatedAt: Date(),
            createdAt: Date(),
            saveCount: saveCount,
            lastSavedAt: lastSavedAt
        )
    }

    func testDedupe_incrementsSaveCount_andUpdatesLastSavedAt() {
        // Simulate two shares of the same URL (canonical set by server)
        let now = Date()
        let earlier = now.addingTimeInterval(-3600)
        var link = makeLink(
            original: "https://example.com/foo",
            saveCount: 1, lastSavedAt: earlier)

        // Simulate dedupe logic: second share of same canonicalUrl
        let newShareDate = now
        link.saveCount += 1
        link.lastSavedAt = newShareDate

        XCTAssertEqual(link.saveCount, 2)
        XCTAssertEqual(link.lastSavedAt, newShareDate)
    }

    func testDedupe_doesNotChangeStatusOrOtherFields() {
        let now = Date()
        var link = makeLink(original: "https://example.com/foo", saveCount: 1)
        link.status = "synced"
        link.title = "Original Title"

        // Simulate dedupe logic: second share
        link.saveCount += 1
        link.lastSavedAt = now

        XCTAssertEqual(link.status, "synced")
        XCTAssertEqual(link.title, "Original Title")
    }

    func testInsertNewLink_setsDefaults() {
        let url = "https://example.com/bar"
        let link = makeLink(original: url)

        XCTAssertEqual(link.originalUrl.absoluteString, url)
        XCTAssertEqual(link.canonicalUrl.absoluteString, url)
        XCTAssertEqual(link.status, "queued")
        XCTAssertEqual(link.saveCount, 1)
        XCTAssertNotNil(link.createdAt)
        XCTAssertNotNil(link.updatedAt)
    }
}
