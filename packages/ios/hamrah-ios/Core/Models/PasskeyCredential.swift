import Foundation

// MARK: - PasskeyCredential Model
//
// Unified / defensive model for passkey (WebAuthn credential) data returned from the
// hamrah web and API services. Handles:
// - snake_case & camelCase variations
// - numeric timestamps (seconds or milliseconds) converted to ISO8601 strings
// - boolean fields encoded as Bool or 0/1 integers
// - optional fields that may not be present in all responses
//
// NOTE:
// If you add new fields in backend/web responses, extend decoding logic here instead
// of scattering decoding fallbacks across views.

public struct PasskeyCredential: Codable, Identifiable {
    public let id: String
    public let name: String
    /// Normalized ISO8601 string for creation date (may be "unknown" if absent)
    public let createdAt: String
    /// Normalized ISO8601 string for last use (optional)
    public let lastUsed: String?
    public let credentialDeviceType: String?
    public let credentialBackedUp: Bool?
    /// Additional raw fields that might be useful later
    public let credentialType: String?
    public let userVerified: Bool?
    public let transports: [String]?
    public let aaguid: String?
    public let counter: Int?

    // MARK: - Custom Decoding

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: DynamicCodingKeys.self)

        // ID (credentialId | id)
        if let credentialId = try? container.decode(String.self, forKey: .init("credentialId")) {
            id = credentialId
        } else if let idValue = try? container.decode(String.self, forKey: .init("id")) {
            id = idValue
        } else {
            throw DecodingError.keyNotFound(
                DynamicCodingKeys("id"),
                .init(codingPath: decoder.codingPath, debugDescription: "Missing credential ID")
            )
        }

        // Name
        name = (try? container.decode(String.self, forKey: .init("name"))) ?? "Passkey"

        // Created At (string or number; camelCase or snake_case)
        createdAt =
            PasskeyCredential.decodeDateString(
                from: container,
                primary: "createdAt",
                secondary: "created_at",
                fallbackLabel: "createdAt"
            ) ?? "unknown"

        // Last Used
        lastUsed = PasskeyCredential.decodeDateString(
            from: container,
            primary: "lastUsed",
            secondary: "last_used",
            fallbackLabel: "lastUsed"
        )

        // Device Type
        credentialDeviceType =
            (try? container.decode(String.self, forKey: .init("credentialDeviceType")))
            ?? (try? container.decode(String.self, forKey: .init("credential_device_type")))

        // Backed Up (bool or int)
        if let boolValue = try? container.decode(Bool.self, forKey: .init("credentialBackedUp")) {
            credentialBackedUp = boolValue
        } else if let boolSnake = try? container.decode(
            Bool.self, forKey: .init("credential_backed_up"))
        {
            credentialBackedUp = boolSnake
        } else if let intValue = try? container.decode(
            Int.self, forKey: .init("credential_backed_up"))
        {
            credentialBackedUp = intValue != 0
        } else if let intCamel = try? container.decode(
            Int.self, forKey: .init("credentialBackedUp"))
        {
            credentialBackedUp = intCamel != 0
        } else {
            credentialBackedUp = nil
        }

        // Optional extended fields (if backend provides them)
        credentialType =
            (try? container.decode(String.self, forKey: .init("credentialType")))
            ?? (try? container.decode(String.self, forKey: .init("credential_type")))
        userVerified =
            (try? container.decode(Bool.self, forKey: .init("userVerified")))
            ?? (try? container.decode(Bool.self, forKey: .init("user_verified")))
        transports =
            (try? container.decode([String].self, forKey: .init("transports")))
            // Sometimes stored as JSON string array - attempt soft decode
            ?? PasskeyCredential.decodeStringifiedStringArray(from: container, key: "transports")
        aaguid =
            (try? container.decode(String.self, forKey: .init("aaguid")))
            // Accept raw base64 array or bytes converted to string (ignore if not convertible)
            ?? PasskeyCredential.decodeAAGUIDFallback(from: container)
        counter =
            (try? container.decode(Int.self, forKey: .init("counter")))
            ?? (try? container.decode(Int.self, forKey: .init("usage_counter")))
    }

    // MARK: - Internal Helpers

    private static func decodeAAGUIDFallback(
        from container: KeyedDecodingContainer<DynamicCodingKeys>
    ) -> String? {
        if let bytes = try? container.decode([UInt8].self, forKey: .init("aaguid")) {
            // Convert to hex or base64 - choose base64 to keep parity with server
            return Data(bytes).base64EncodedString()
        }
        return nil
    }

    /// Attempts to decode a date that may be:
    /// - A string (already ISO-like)
    /// - A Double or Int representing seconds or milliseconds
    private static func decodeDateString(
        from container: KeyedDecodingContainer<DynamicCodingKeys>,
        primary: String,
        secondary: String?,
        fallbackLabel: String
    ) -> String? {
        // 1. Try string (primary)
        if let s = try? container.decode(String.self, forKey: .init(primary)) {
            return s
        }
        // 2. Try string (secondary)
        if let secondary = secondary,
            let s2 = try? container.decode(String.self, forKey: .init(secondary))
        {
            return s2
        }
        // 3. Try numeric (primary)
        if let n = try? container.decode(Double.self, forKey: .init(primary)) {
            return isoString(fromNumericTimestamp: n)
        }
        if let nInt = try? container.decode(Int.self, forKey: .init(primary)) {
            return isoString(fromNumericTimestamp: Double(nInt))
        }
        // 4. Try numeric (secondary)
        if let secondary = secondary,
            let n2 = try? container.decode(Double.self, forKey: .init(secondary))
        {
            return isoString(fromNumericTimestamp: n2)
        }
        if let secondary = secondary,
            let n2Int = try? container.decode(Int.self, forKey: .init(secondary))
        {
            return isoString(fromNumericTimestamp: Double(n2Int))
        }
        // Nothing found
        return nil
    }

    /// Heuristic conversion: if numeric value looks like milliseconds, convert accordingly.
    private static func isoString(fromNumericTimestamp raw: Double) -> String {
        let thresholdSecondsFarFuture: Double = 3_252_460_800  // ~01-01-2073 in seconds
        let asSeconds: TimeInterval
        if raw > thresholdSecondsFarFuture {
            // Probably ms
            asSeconds = raw / 1000.0
        } else {
            asSeconds = raw
        }
        let date = Date(timeIntervalSince1970: asSeconds)
        return ISO8601DateFormatter().string(from: date)
    }

    private static func decodeStringifiedStringArray(
        from container: KeyedDecodingContainer<DynamicCodingKeys>,
        key: String
    ) -> [String]? {
        guard let raw = try? container.decode(String.self, forKey: .init(key)) else {
            return nil
        }
        guard let data = raw.data(using: .utf8) else { return nil }
        if let arr = try? JSONDecoder().decode([String].self, from: data) {
            return arr
        }
        return nil
    }
}

// MARK: - DynamicCodingKeys
//
// Enables decoding of arbitrarily named keys (camelCase + snake_case)
public struct DynamicCodingKeys: CodingKey, Hashable {
    public let stringValue: String
    public let intValue: Int?

    public init?(stringValue: String) {
        self.stringValue = stringValue
        self.intValue = nil
    }

    public init?(intValue: Int) {
        self.stringValue = String(intValue)
        self.intValue = intValue
    }

    public init(_ string: String) {
        self.stringValue = string
        self.intValue = nil
    }
}

// MARK: - PasskeyListResponse
//
// Standard wrapper returned by listing endpoints:
// {
//   "success": true,
//   "credentials": [ ... ],
//   "error": null
// }
public struct PasskeyListResponse: Codable {
    public let success: Bool
    public let credentials: [PasskeyCredential]
    public let error: String?

    public init(success: Bool, credentials: [PasskeyCredential], error: String?) {
        self.success = success
        self.credentials = credentials
        self.error = error
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.success = (try? container.decode(Bool.self, forKey: .success)) ?? false
        self.credentials =
            (try? container.decode([PasskeyCredential].self, forKey: .credentials)) ?? []
        self.error = try? container.decode(String.self, forKey: .error)
    }
}

// MARK: - Convenience Extensions

extension Array where Element == PasskeyCredential {
    /// Sort credentials by creation date descending (best effort)
    public func sortedByCreatedDescending() -> [PasskeyCredential] {
        let formatter = ISO8601DateFormatter()
        return sorted { lhs, rhs in
            guard
                let lDate = formatter.date(from: lhs.createdAt),
                let rDate = formatter.date(from: rhs.createdAt)
            else { return false }
            return lDate > rDate
        }
    }
}
