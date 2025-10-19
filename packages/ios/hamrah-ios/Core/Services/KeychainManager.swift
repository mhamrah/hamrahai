import Foundation
import Security

class KeychainManager {
    static let shared = KeychainManager()

    private init() {}

    private let service = "com.hamrah.app"
    // Shared Keychain access group for app + extension (optional)
    // Uses AppIdentifierPrefix injected by codesign to form the full identifier, e.g. "TEAMID.app.hamrah.ios"
    // When present, items will be written to/read from this shared access group.
    private let accessGroup: String? = nil // AppAccessGroup.value - temporarily disabled due to build issue

    // MARK: - Generic Keychain Operations

    func store(_ data: Data, for key: String) -> Bool {
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
        ]
        if let accessGroup = accessGroup {
            query[kSecAttrAccessGroup as String] = accessGroup
        }

        // Delete any existing item first
        SecItemDelete(query as CFDictionary)

        // Add the new item
        let status = SecItemAdd(query as CFDictionary, nil)
        return status == errSecSuccess
    }

    func retrieve(for key: String) -> Data? {
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        if let accessGroup = accessGroup {
            query[kSecAttrAccessGroup as String] = accessGroup
        }

        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)

        if status == errSecSuccess {
            return result as? Data
        }
        return nil
    }

    func delete(for key: String) -> Bool {
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
        ]
        if let accessGroup = accessGroup {
            query[kSecAttrAccessGroup as String] = accessGroup
        }

        let status = SecItemDelete(query as CFDictionary)
        return status == errSecSuccess || status == errSecItemNotFound
    }

    // MARK: - String Convenience Methods

    func store(_ string: String, for key: String) -> Bool {
        guard let data = string.data(using: .utf8) else { return false }
        return store(data, for: key)
    }

    func retrieveString(for key: String) -> String? {
        guard let data = retrieve(for: key) else { return nil }
        return String(data: data, encoding: .utf8)
    }

    // MARK: - Boolean Convenience Methods

    func store(_ bool: Bool, for key: String) -> Bool {
        let data = Data([bool ? 1 : 0])
        return store(data, for: key)
    }

    func retrieveBool(for key: String) -> Bool? {
        guard let data = retrieve(for: key), !data.isEmpty else { return nil }
        return data[0] == 1
    }

    // MARK: - Double Convenience Methods

    func store(_ double: Double, for key: String) -> Bool {
        let data = withUnsafeBytes(of: double) { Data($0) }
        return store(data, for: key)
    }

    func retrieveDouble(for key: String) -> Double? {
        guard let data = retrieve(for: key), data.count == MemoryLayout<Double>.size else {
            return nil
        }
        return data.withUnsafeBytes { $0.load(as: Double.self) }
    }

    // MARK: - Clear All Hamrah Data

    func clearAllHamrahData() -> Bool {
        let keys = [
            "hamrah_user",
            "hamrah_access_token",
            "hamrah_refresh_token",
            "hamrah_is_authenticated",
            "hamrah_auth_timestamp",
            "hamrah_token_expires_at",
        ]

        var allSuccess = true
        for key in keys {
            if !delete(for: key) {
                allSuccess = false
            }
        }
        return allSuccess
    }
}
