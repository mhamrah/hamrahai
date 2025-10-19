import Combine
import Foundation

#if os(iOS)
    import DeviceCheck
#endif

class SecureAPIService: ObservableObject {
    let objectWillChange = ObservableObjectPublisher()
    static let shared = SecureAPIService()

    private let attestationManager = AppAttestationManager.shared
    private var baseURL: String {
        APIConfiguration.shared.baseURL
    }

    private init() {}

    // MARK: - Secure API Request Methods

    /// Makes a secure API request with App Attestation
    func makeSecureRequest<T: Codable>(
        endpoint: String,
        method: HTTPMethod = .GET,
        body: [String: Any]? = nil,
        accessToken: String?,
        responseType: T.Type,
        customBaseURL: String? = nil,
        isRetry: Bool = false
    ) async throws -> T {
        let targetBaseURL = customBaseURL ?? baseURL
        let url = URL(string: "\(targetBaseURL)\(endpoint)")!
        var request = URLRequest(url: url)
        request.httpMethod = method.rawValue
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        // Attach tracing headers
        let traceId = UUID().uuidString.lowercased()
        request.setValue(traceId, forHTTPHeaderField: "X-Trace-Id")
        if let token = accessToken, let userId = decodeJWTSubject(from: token) {
            request.setValue(userId, forHTTPHeaderField: "X-User-Id")
        }

        // Add authorization if provided
        if let token = accessToken {
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }

        // Add body if provided
        if let body = body {
            request.httpBody = try JSONSerialization.data(withJSONObject: body)
        }

        // Generate challenge for attestation
        let challenge = generateRequestChallenge(url: url, method: method, body: body)

        // Add App Attestation headers (required)
        let attestationHeaders = try await attestationManager.generateAttestationHeaders(
            for: challenge)
        for (key, value) in attestationHeaders {
            request.setValue(value, forHTTPHeaderField: key)
        }

        // Add challenge for server verification
        request.setValue(challenge.base64EncodedString(), forHTTPHeaderField: "X-Request-Challenge")

        let (data, response) = try await URLSession.shared.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse else {
            throw APIError.invalidResponse
        }

        // Handle authentication errors with attestation retry
        if httpResponse.statusCode == 401 {
            // On 401, attempt to re-initialize attestation once
            if !isRetry, let token = accessToken {
                print("⚠️ 401 Unauthorized - Attempting to re-initialize attestation")

                // Clear attestation flag and retry initialization
                attestationManager.clearAttestationFlag()

                do {
                    try await attestationManager.initializeAttestation(accessToken: token)
                    print("✅ Attestation re-initialized, retrying request")

                    // Retry the request once
                    return try await makeSecureRequest(
                        endpoint: endpoint,
                        method: method,
                        body: body,
                        accessToken: accessToken,
                        responseType: responseType,
                        customBaseURL: customBaseURL,
                        isRetry: true  // Prevent infinite retry loop
                    )
                } catch {
                    print("❌ Attestation re-initialization failed: \(error)")
                }
            }
            throw APIError.unauthorized
        }

        guard httpResponse.statusCode == 200 else {
            // Try to decode error message
            if let errorData = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                let errorMessage = errorData["error"] as? String
            {
                throw APIError.serverError(httpResponse.statusCode, errorMessage)
            }
            throw APIError.serverError(httpResponse.statusCode, "Request failed")
        }

        return try JSONDecoder().decode(T.self, from: data)
    }

    /// Initialize attestation (should be called after login)
    func initializeAttestation(accessToken: String) async {
        do {
            try await attestationManager.initializeAttestation(accessToken: accessToken)
            print("✅ App Attestation initialized successfully")
        } catch {
            print("⚠️ Failed to initialize App Attestation: \(error)")

            #if os(iOS)
                // If we get a DCError code 2 (invalid key), try a force reset and retry once
                if let dcError = error as? DCError, dcError.code.rawValue == 2 {
                    print("🔄 Detected DCError code 2 - attempting force reset and retry...")
                    attestationManager.forceReset()

                    do {
                        try await attestationManager.initializeAttestation(accessToken: accessToken)
                        print("✅ App Attestation initialized successfully after reset")
                    } catch {
                        print("⚠️ App Attestation still failing after reset: \(error)")
                        print("💡 Continuing with fallback headers - app will still work")
                    }
                }
            #endif
            // Continue without attestation - app should still work with fallback headers
        }
    }

    /// Debug function to diagnose and potentially fix App Attestation issues
    func debugAppAttestation(accessToken: String) async {
        print("🔍 === App Attestation Debug Session ===")

        // First, diagnose current state
        attestationManager.diagnoseState()

        // Try initialization and see what happens
        print("🔍 Attempting initialization...")
        do {
            try await attestationManager.initializeAttestation(accessToken: accessToken)
            print("✅ Debug: App Attestation initialization succeeded")
        } catch {
            print("❌ Debug: App Attestation initialization failed: \(error)")

            // If it fails, try a force reset and retry
            print("🔄 Debug: Attempting force reset and retry...")
            attestationManager.forceReset()

            do {
                try await attestationManager.initializeAttestation(accessToken: accessToken)
                print("✅ Debug: App Attestation initialization succeeded after reset")
            } catch {
                print("❌ Debug: App Attestation still failing after reset: \(error)")
                print("💡 This may indicate a device/environment issue or rate limiting")
            }
        }

        print("🔍 === End Debug Session ===")
    }

    // MARK: - Convenience Methods

    func get<T: Codable>(
        endpoint: String,
        accessToken: String?,
        responseType: T.Type,
        customBaseURL: String? = nil
    ) async throws -> T {
        return try await makeSecureRequest(
            endpoint: endpoint,
            method: .GET,
            body: nil,
            accessToken: accessToken,
            responseType: responseType,
            customBaseURL: customBaseURL
        )
    }

    func post<T: Codable>(
        endpoint: String,
        body: [String: Any],
        accessToken: String?,
        responseType: T.Type,
        customBaseURL: String? = nil
    ) async throws -> T {
        return try await makeSecureRequest(
            endpoint: endpoint,
            method: .POST,
            body: body,
            accessToken: accessToken,
            responseType: responseType,
            customBaseURL: customBaseURL
        )
    }

    func put<T: Codable>(
        endpoint: String,
        body: [String: Any],
        accessToken: String?,
        responseType: T.Type,
        customBaseURL: String? = nil
    ) async throws -> T {
        return try await makeSecureRequest(
            endpoint: endpoint,
            method: .PUT,
            body: body,
            accessToken: accessToken,
            responseType: responseType,
            customBaseURL: customBaseURL
        )
    }

    func delete<T: Codable>(
        endpoint: String,
        body: [String: Any]? = nil,
        accessToken: String?,
        responseType: T.Type
    ) async throws -> T {
        return try await makeSecureRequest(
            endpoint: endpoint,
            method: .DELETE,
            body: body,
            accessToken: accessToken,
            responseType: responseType
        )
    }

    // MARK: - Raw HEAD/Download (binary/HTML)

    /// Issues a raw HEAD request with App Attestation.
    /// Returns the HTTPURLResponse so callers can inspect headers like ETag/Content-Length.
    /// Accepts 200 OK and 304 Not Modified statuses.
    func headRaw(
        endpoint: String,
        ifNoneMatchETag: String? = nil,
        accessToken: String?,
        customBaseURL: String? = nil,
        isRetry: Bool = false
    ) async throws -> HTTPURLResponse {
        let targetBaseURL = customBaseURL ?? baseURL
        let url = URL(string: "\(targetBaseURL)\(endpoint)")!
        var request = URLRequest(url: url)
        request.httpMethod = HTTPMethod.HEAD.rawValue

        // Attach tracing headers
        let traceId = UUID().uuidString.lowercased()
        request.setValue(traceId, forHTTPHeaderField: "X-Trace-Id")
        if let token = accessToken, let userId = decodeJWTSubject(from: token) {
            request.setValue(userId, forHTTPHeaderField: "X-User-Id")
        }

        // Authorization (optional)
        if let token = accessToken {
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }
        // Conditional request (optional)
        if let etag = ifNoneMatchETag {
            request.setValue(etag, forHTTPHeaderField: "If-None-Match")
        }

        // App Attestation
        let challenge = generateRequestChallenge(url: url, method: .HEAD, body: nil)
        let attestationHeaders = try await attestationManager.generateAttestationHeaders(
            for: challenge)
        for (key, value) in attestationHeaders {
            request.setValue(value, forHTTPHeaderField: key)
        }
        request.setValue(challenge.base64EncodedString(), forHTTPHeaderField: "X-Request-Challenge")

        // Execute
        let (_, response) = try await URLSession.shared.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse else {
            throw APIError.invalidResponse
        }

        if httpResponse.statusCode == 401 {
            if !isRetry, let token = accessToken {
                print("⚠️ 401 Unauthorized - Attempting to re-initialize attestation")
                attestationManager.clearAttestationFlag()
                do {
                    try await attestationManager.initializeAttestation(accessToken: token)
                    print("✅ Attestation re-initialized, retrying request")
                    return try await headRaw(
                        endpoint: endpoint,
                        ifNoneMatchETag: ifNoneMatchETag,
                        accessToken: accessToken,
                        customBaseURL: customBaseURL,
                        isRetry: true
                    )
                } catch {
                    print("❌ Attestation re-initialization failed: \(error)")
                }
            }
            throw APIError.unauthorized
        }

        guard httpResponse.statusCode == 200 || httpResponse.statusCode == 304 else {
            throw APIError.serverError(httpResponse.statusCode, "HEAD request failed")
        }
        return httpResponse
    }

    /// Downloads raw content (binary/HTML) with App Attestation.
    /// Returns the temporary file URL and HTTPURLResponse for caller-managed caching.
    func downloadRaw(
        endpoint: String,
        accessToken: String?,
        customBaseURL: String? = nil,
        isRetry: Bool = false
    ) async throws -> (URL, HTTPURLResponse) {
        let targetBaseURL = customBaseURL ?? baseURL
        let url = URL(string: "\(targetBaseURL)\(endpoint)")!
        var request = URLRequest(url: url)
        request.httpMethod = HTTPMethod.GET.rawValue

        // Attach tracing headers
        let traceId = UUID().uuidString.lowercased()
        request.setValue(traceId, forHTTPHeaderField: "X-Trace-Id")
        if let token = accessToken, let userId = decodeJWTSubject(from: token) {
            request.setValue(userId, forHTTPHeaderField: "X-User-Id")
        }

        // Authorization (optional)
        if let token = accessToken {
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }

        // App Attestation
        let challenge = generateRequestChallenge(url: url, method: .GET, body: nil)
        let attestationHeaders = try await attestationManager.generateAttestationHeaders(
            for: challenge)
        for (key, value) in attestationHeaders {
            request.setValue(value, forHTTPHeaderField: key)
        }
        request.setValue(challenge.base64EncodedString(), forHTTPHeaderField: "X-Request-Challenge")

        // Execute download
        let (tempURL, response) = try await URLSession.shared.download(for: request)

        guard let httpResponse = response as? HTTPURLResponse else {
            throw APIError.invalidResponse
        }

        if httpResponse.statusCode == 401 {
            if !isRetry, let token = accessToken {
                print("⚠️ 401 Unauthorized - Attempting to re-initialize attestation")
                attestationManager.clearAttestationFlag()
                do {
                    try await attestationManager.initializeAttestation(accessToken: token)
                    print("✅ Attestation re-initialized, retrying request")
                    return try await downloadRaw(
                        endpoint: endpoint,
                        accessToken: accessToken,
                        customBaseURL: customBaseURL,
                        isRetry: true
                    )
                } catch {
                    print("❌ Attestation re-initialization failed: \(error)")
                }
            }
            throw APIError.unauthorized
        }

        guard httpResponse.statusCode == 200 else {
            throw APIError.serverError(httpResponse.statusCode, "Download request failed")
        }

        return (tempURL, httpResponse)
    }

    // MARK: - Private Methods

    private func generateRequestChallenge(url: URL, method: HTTPMethod, body: [String: Any]?)
        -> Data
    {
        // Create a deterministic challenge based on request details
        var challengeString = "\(method.rawValue):\(url.absoluteString)"

        if let body = body {
            // Sort keys for deterministic serialization
            let sortedKeys = body.keys.sorted()
            let bodyString = sortedKeys.map { key in
                "\(key):\(body[key] ?? "")"
            }.joined(separator: ",")
            challengeString += ":\(bodyString)"
        }

        challengeString += ":\(Date().timeIntervalSince1970)"

        return challengeString.data(using: .utf8) ?? Data()
    }

    // Extracts `sub` (user id) from a JWT access token (base64url JSON payload).
    private func decodeJWTSubject(from token: String) -> String? {
        let parts = token.split(separator: ".")
        guard parts.count >= 2,
            let payloadData = base64URLDecode(String(parts[1])),
            let json = try? JSONSerialization.jsonObject(with: payloadData) as? [String: Any]
        else { return nil }
        return json["sub"] as? String
    }

    // Base64URL decoder used for JWT payloads.
    private func base64URLDecode(_ input: String) -> Data? {
        var base64 = input.replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        let padding = 4 - (base64.count % 4)
        if padding < 4 {
            base64.append(String(repeating: "=", count: padding))
        }
        return Data(base64Encoded: base64)
    }
}

// MARK: - HTTP Method Enum

enum HTTPMethod: String {
    case GET = "GET"
    case HEAD = "HEAD"
    case POST = "POST"
    case PUT = "PUT"
    case DELETE = "DELETE"
    case PATCH = "PATCH"
}

// MARK: - API Error Types

enum APIError: LocalizedError {
    case invalidResponse
    case unauthorized
    case serverError(Int, String)
    case attestationFailed(String)
    case simulatorNotSupported

    var errorDescription: String? {
        switch self {
        case .invalidResponse:
            return "Invalid response from server"
        case .unauthorized:
            return "Authentication required. Please sign in again."
        case .serverError(let code, let message):
            return "Server error (\(code)): \(message)"
        case .attestationFailed(let details):
            #if targetEnvironment(simulator)
                return
                    "App verification not supported on simulator. Please test on a physical device."
            #else
                return "App verification failed: \(details)"
            #endif
        case .simulatorNotSupported:
            return
                "This feature requires a physical iOS device and is not supported on the simulator."
        }
    }
}

// MARK: - Note:
// APIResponse model now lives in Models/APIResponse.swift (previously referenced from MyAccountView.swift)
