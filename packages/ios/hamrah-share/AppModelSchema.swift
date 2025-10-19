//
//  AppModelSchema.swift
//  hamrah-ios
//
//  Unified SwiftData schema and container helpers used by the app,
//  background sync, and share extension to guarantee schema consistency.
//

import Foundation
import SwiftData

enum AppModelSchema {
    /// App Group identifier shared by the main app and extensions.
    static let appGroupId = "group.app.hamrah.ios"

    /// Canonical SwiftData schema for all models.
    /// Keep this list ordered and consistent to avoid migration instability.
    static let schema: Schema = Schema([
        LinkEntity.self,
        TagEntity.self,
        SyncCursor.self,
        UserPrefs.self,
    ])

    /// Create a shared ModelContainer stored in the App Group container.
    static func makeSharedContainer() throws -> ModelContainer {
        let config = ModelConfiguration(groupContainer: .identifier(appGroupId))
        return try ModelContainer(for: schema, configurations: config)
    }

    /// Create an in-memory ModelContainer (useful for previews/tests).
    static func makeInMemoryContainer() -> ModelContainer {
        let config = ModelConfiguration(isStoredInMemoryOnly: true)
        return try! ModelContainer(for: schema, configurations: config)
    }

    /// Convenience helper to obtain a ModelContext from the shared container.
    static func makeSharedContext() throws -> ModelContext {
        ModelContext(try makeSharedContainer())
    }

    #if DEBUG
        /// Development-only helper that attempts to recover from schema incompatibility
        /// by removing existing store files and recreating the container.
        static func makeSharedContainerWithRecovery() -> ModelContainer {
            do {
                return try makeSharedContainer()
            } catch {
                removeDefaultStoreFiles()
                return (try? makeSharedContainer()) ?? makeInMemoryContainer()
            }
        }

        /// Removes the default SwiftData store files from the App Group container.
        private static func removeDefaultStoreFiles() {
            guard
                let baseURL = FileManager.default
                    .containerURL(forSecurityApplicationGroupIdentifier: appGroupId)?
                    .appendingPathComponent("Library/Application Support", isDirectory: true)
            else { return }

            let filenames = ["default.store", "default.store-wal", "default.store-shm"]
            let fm = FileManager.default
            for name in filenames {
                let url = baseURL.appendingPathComponent(name)
                _ = try? fm.removeItem(at: url)
            }
        }
    #endif
}
