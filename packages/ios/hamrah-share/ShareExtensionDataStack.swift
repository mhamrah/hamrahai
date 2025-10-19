import Foundation
import SwiftData
import os

/// Provides a SwiftData ModelContainer for the Share Extension that stores data
/// in the shared App Group container so the main app and extension can access
/// the same database.
final class ShareExtensionDataStack {
    private static let logger = Logger(subsystem: "app.hamrah.ios.share", category: "DataStack")

    /// Uses AppModelSchema for a unified schema shared with the main app.

    /// A shared ModelContainer configured to store models in the App Group.
    /// This allows the share extension to write queued links while offline and
    /// the main app to read/sync them when available.
    static let shared: ModelContainer = {
        let logger = ShareExtensionDataStack.logger
        #if DEBUG
            logger.log(
                "ShareExtensionDataStack: Building ModelContainer [DEBUG] for app group \(AppModelSchema.appGroupId, privacy: .public)"
            )
        #else
            logger.log(
                "ShareExtensionDataStack: Building ModelContainer [RELEASE] for app group \(AppModelSchema.appGroupId, privacy: .public)"
            )
        #endif

        if let url = FileManager.default.containerURL(
            forSecurityApplicationGroupIdentifier: AppModelSchema.appGroupId)
        {
            logger.log(
                "ShareExtensionDataStack: App Group URL: \(url.absoluteString, privacy: .public)")
        } else {
            logger.error("ShareExtensionDataStack: Failed to resolve App Group URL")
        }

        #if DEBUG
            let container = AppModelSchema.makeSharedContainerWithRecovery()
            logger.log("ShareExtensionDataStack: ModelContainer ready (with recovery)")
            return container
        #else
            do {
                let container = try AppModelSchema.makeSharedContainer()
                logger.log("ShareExtensionDataStack: ModelContainer ready")
                return container
            } catch {
                logger.error(
                    "ShareExtensionDataStack: Failed to create shared container: \(error.localizedDescription, privacy: .public)"
                )
                let fallback = AppModelSchema.makeInMemoryContainer()
                logger.error("ShareExtensionDataStack: Falling back to in-memory container")
                return fallback
            }
        #endif
    }()

    /// Convenience accessor for the main ModelContext.
    static var mainContext: ModelContext {
        ShareExtensionDataStack.logger.log(
            "ShareExtensionDataStack: Creating ModelContext from shared container")
        return ModelContext(shared)
    }
}
