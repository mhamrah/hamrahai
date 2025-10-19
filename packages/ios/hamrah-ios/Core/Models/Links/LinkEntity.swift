import Foundation
import SwiftData

@Model
final class LinkEntity {
    // MARK: - Identifiers
    @Attribute(.unique) var localId: UUID = UUID()
    var serverId: String?  // ULID from server

    // MARK: - URLs
    var originalUrl: URL
    var canonicalUrl: URL  // Server-controlled canonical URL; initially equals originalUrl until sync replaces it

    // MARK: - Metadata
    var title: String?
    var snippet: String?
    var sourceApp: String?
    var sharedText: String?
    var sharedAt: Date

    // MARK: - State
    var status: String  // {queued, syncing, synced, failed}
    var attempts: Int = 0
    var lastError: String?
    var saveCount: Int = 1
    var lastSavedAt: Date?
    var updatedAt: Date
    var createdAt: Date

    // MARK: - Summaries & Tags
    var summaryShort: String?
    var summaryLong: String?
    var lang: String?
    @Relationship(deleteRule: .cascade, inverse: \TagEntity.links)
    var tags: [TagEntity] = []

    // MARK: - Init
    init(
        originalUrl: URL,
        canonicalUrl: URL,
        title: String? = nil,
        snippet: String? = nil,
        sourceApp: String? = nil,
        sharedText: String? = nil,
        sharedAt: Date = Date(),
        status: String = "queued",
        attempts: Int = 0,
        lastError: String? = nil,
        saveCount: Int = 1,
        lastSavedAt: Date? = nil,
        updatedAt: Date = Date(),
        createdAt: Date = Date(),
        summaryShort: String? = nil,
        summaryLong: String? = nil,
        lang: String? = nil,
        tags: [TagEntity] = [],

        serverId: String? = nil
    ) {
        self.originalUrl = originalUrl
        self.canonicalUrl = canonicalUrl
        self.title = title
        self.snippet = snippet
        self.sourceApp = sourceApp
        self.sharedText = sharedText
        self.sharedAt = sharedAt
        self.status = status
        self.attempts = attempts
        self.lastError = lastError
        self.saveCount = saveCount
        self.lastSavedAt = lastSavedAt
        self.updatedAt = updatedAt
        self.createdAt = createdAt
        self.summaryShort = summaryShort
        self.summaryLong = summaryLong
        self.lang = lang
        self.tags = tags

        self.serverId = serverId
    }
}
