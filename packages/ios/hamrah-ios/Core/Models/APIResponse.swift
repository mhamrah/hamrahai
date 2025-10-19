import Foundation

/// APIResponse
/// Lightweight, defensive wrapper for simple success/error style backend endpoints.
/// Currently used by:
///  - Passkey registration verification
///  - Passkey deletion
///  - (Future) generic mutation endpoints
///
/// The backend may (now or in the future) return any of:
/// {
///   "success": true,
///   "error": null
/// }
/// {
///   "success": false,
///   "error": "Something went wrong"
/// }
/// {
///   "ok": true,
///   "message": "Created"
/// }
/// {
///   "success": true,
///   "detail": "Optional detail text"
/// }
///
/// This model tolerates optional / alternative field names to reduce coupling.
public struct APIResponse: Codable {

    /// Whether the operation succeeded.
    public let success: Bool

    /// Canonical error message (nil when `success == true`)
    public let error: String?

    /// Optional backend-provided informational message (success or failure).
    public let message: String?

    /// Optional detail / description field (sometimes used instead of `message`).
    public let detail: String?

    /// Raw backing store in case future fields are added and needed ad‑hoc.
    public let raw: [String: AnyCodable]

    // MARK: - Convenience

    /// True when an error string is present.
    public var hasError: Bool { error != nil }

    /// Unified human‑readable text (prefers error, then message, then detail).
    public var displayText: String? {
        error ?? message ?? detail
    }

    // MARK: - Coding

    public init(
        success: Bool,
        error: String? = nil,
        message: String? = nil,
        detail: String? = nil,
        raw: [String: AnyCodable] = [:]
    ) {
        self.success = success
        self.error = error
        self.message = message
        self.detail = detail
        self.raw = raw
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: APIDynamicCodingKeys.self)

        // Capture all key/value pairs first
        var tempRaw: [String: AnyCodable] = [:]
        for key in container.allKeys {
            if let value = try? container.decode(AnyCodable.self, forKey: key) {
                tempRaw[key.stringValue] = value
            }
        }

        // Success fallbacks: "success" (preferred) or "ok"
        let successValue =
            (try? container.decode(Bool.self, forKey: APIDynamicCodingKeys("success")))
            ?? (try? container.decode(Bool.self, forKey: APIDynamicCodingKeys("ok"))) ?? false

        // Error fallbacks
        let errorValue =
            (try? container.decode(String.self, forKey: APIDynamicCodingKeys("error")))
            ?? (try? container.decode(String.self, forKey: APIDynamicCodingKeys("err"))) ?? nil

        // Message / detail fallbacks
        let messageValue =
            (try? container.decode(String.self, forKey: APIDynamicCodingKeys("message")))
            ?? (try? container.decode(String.self, forKey: APIDynamicCodingKeys("msg"))) ?? nil

        let detailValue =
            (try? container.decode(String.self, forKey: APIDynamicCodingKeys("detail")))
            ?? (try? container.decode(String.self, forKey: APIDynamicCodingKeys("description")))
            ?? nil

        self.success = successValue
        self.error = errorValue
        self.message = messageValue
        self.detail = detailValue
        self.raw = tempRaw
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: APIDynamicCodingKeys.self)
        try container.encode(success, forKey: APIDynamicCodingKeys("success"))
        if let error = error { try container.encode(error, forKey: APIDynamicCodingKeys("error")) }
        if let message = message {
            try container.encode(message, forKey: APIDynamicCodingKeys("message"))
        }
        if let detail = detail {
            try container.encode(detail, forKey: APIDynamicCodingKeys("detail"))
        }

        // Persist any additional raw values that are not the canonical fields
        for (k, v) in raw {
            if ["success", "error", "message", "detail"].contains(k) { continue }
            try container.encode(v, forKey: APIDynamicCodingKeys(k))
        }
    }
}

// MARK: - DynamicCodingKeys

private struct APIDynamicCodingKeys: CodingKey, Hashable {
    let stringValue: String
    let intValue: Int?

    init(_ string: String) {
        self.stringValue = string
        self.intValue = nil
    }

    init?(stringValue: String) {
        self.init(stringValue)
    }

    init?(intValue: Int) {
        self.stringValue = String(intValue)
        self.intValue = intValue
    }
}

// MARK: - AnyCodable (lightweight embedded implementation)

/// A type-erased Codable wrapper allowing us to retain unknown fields without
/// introducing a third-party dependency.
public struct AnyCodable: Codable {
    public let value: Any

    public init(_ value: Any) { self.value = value }

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()

        if container.decodeNil() {
            self.value = Optional<Any>.none as Any
        } else if let b = try? container.decode(Bool.self) {
            self.value = b
        } else if let i = try? container.decode(Int.self) {
            self.value = i
        } else if let d = try? container.decode(Double.self) {
            self.value = d
        } else if let s = try? container.decode(String.self) {
            self.value = s
        } else if let arr = try? container.decode([AnyCodable].self) {
            self.value = arr.map { $0.value }
        } else if let dict = try? container.decode([String: AnyCodable].self) {
            self.value = dict.mapValues { $0.value }
        } else {
            throw DecodingError.dataCorruptedError(
                in: container,
                debugDescription: "Unsupported JSON value")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()

        switch value {
        case Optional<Any>.none:
            try container.encodeNil()
        case let b as Bool:
            try container.encode(b)
        case let i as Int:
            try container.encode(i)
        case let d as Double:
            try container.encode(d)
        case let s as String:
            try container.encode(s)
        case let arr as [Any]:
            try container.encode(arr.map { AnyCodable($0) })
        case let dict as [String: Any]:
            try container.encode(dict.mapValues { AnyCodable($0) })
        default:
            let context = EncodingError.Context(
                codingPath: container.codingPath,
                debugDescription: "Unsupported JSON value")
            throw EncodingError.invalidValue(value, context)
        }
    }
}
