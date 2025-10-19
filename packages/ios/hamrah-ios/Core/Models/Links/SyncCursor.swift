import Foundation
import SwiftData

/// Singleton model to track sync state with the server.
@Model
final class SyncCursor {
    /// There should only ever be one SyncCursor instance in the store.
    @Attribute(.unique) var id: Int = 0

    /// The last server-provided cursor for delta sync.
    var lastUpdatedCursor: String?

    /// The last time a full sync was performed.
    var lastFullSyncAt: Date?

    init(
        lastUpdatedCursor: String? = nil,
        lastFullSyncAt: Date? = nil
    ) {
        self.lastUpdatedCursor = lastUpdatedCursor
        self.lastFullSyncAt = lastFullSyncAt
    }
}
