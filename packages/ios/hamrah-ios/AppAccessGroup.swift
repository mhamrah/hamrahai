import Foundation

struct AppAccessGroup {
    /// Returns the shared Keychain access group identifier, or nil if not available.
    static var value: String? {
        (Bundle.main.object(forInfoDictionaryKey: "AppIdentifierPrefix") as? String).map {
            "\($0)app.hamrah.ios"
        }
    }
}
