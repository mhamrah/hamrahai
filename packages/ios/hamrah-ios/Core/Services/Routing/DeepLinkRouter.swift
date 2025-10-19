import Foundation
import OSLog

/// Centralized deep link router for the app.
/// Supports custom scheme links like:
/// - hamrah://sync
/// - hamrah:///sync
/// - hamrah://?action=sync
///
/// Extend this router to add more deep link actions as needed.
enum DeepLink {
    case sync(reason: String? = nil)
}

final class DeepLinkRouter {
    private static let logger = Logger(subsystem: "app.hamrah.ios", category: "DeepLinkRouter")

    /// Entry point for handling incoming URLs.
    /// - Parameter url: The URL received by the application.
    /// - Returns: true if the link was recognized and handled; false otherwise.
    @discardableResult
    static func handle(_ url: URL) -> Bool {
        guard let deepLink = parseDeepLink(from: url) else {
            logger.debug("Unrecognized deep link: \(url.absoluteString, privacy: .public)")
            return false
        }
        return handle(deepLink)
    }

    /// Parses a URL into a DeepLink enum if recognized.
    private static func parseDeepLink(from url: URL) -> DeepLink? {
        // Must be our custom scheme
        guard url.scheme?.lowercased() == "hamrah" else { return nil }

        // Support multiple formats:
        // 1) hamrah://sync                 => host == "sync"
        // 2) hamrah:///sync                => pathComponents contains "sync"
        // 3) hamrah://?action=sync         => queryItems contain action=sync
        let host = url.host?.lowercased()
        let pathComponents = url.pathComponents.filter { $0 != "/" }.map { $0.lowercased() }
        let actionQuery = URLComponents(url: url, resolvingAgainstBaseURL: false)?
            .queryItems?
            .first(where: { $0.name.lowercased() == "action" })?
            .value?
            .lowercased()

        // Prefer explicit host match first
        if host == "sync" { return .sync(reason: "deeplink_host") }

        // Then check for path-based action
        if let first = pathComponents.first, first == "sync" {
            return .sync(reason: "deeplink_path")
        }

        // Finally, query param action
        if actionQuery == "sync" {
            return .sync(reason: "deeplink_query")
        }

        return nil
    }

    /// Routes the parsed deep link into the appropriate feature.
    private static func handle(_ deepLink: DeepLink) -> Bool {
        switch deepLink {
        case .sync(let reason):
            // Fire-and-forget sync trigger; SyncEngine manages its own queue.
            let r = reason ?? "deeplink"
            logger.info("Handling deep link: sync; reason=\(r, privacy: .public)")
            SyncEngine().triggerSync(reason: r)
            return true
        }
    }
}
