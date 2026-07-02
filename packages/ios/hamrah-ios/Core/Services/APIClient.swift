import CryptoKit
import Foundation

#if os(iOS)
    import DeviceCheck
#endif

enum APIAuthRequirement {
    case none
    case optional
    case required
    case bootstrap
}

enum HamrahAPIError: LocalizedError, Equatable {
    case unauthorized
    case sessionExpired
    case server(Int, String)
    case network(String)
    case decoding(String)
    case attestation(String)
    case invalidRequest(String)

    var errorDescription: String? {
        switch self {
        case .unauthorized:
            return "Authentication required. Please sign in again."
        case .sessionExpired:
            return "Session expired. Please sign in again."
        case .server(let code, let message):
            return "Server error (\(code)): \(message)"
        case .network(let message):
            return "Network error: \(message)"
        case .decoding(let message):
            return "Response decoding failed: \(message)"
        case .attestation(let message):
            return "App verification failed: \(message)"
        case .invalidRequest(let message):
            return "Invalid request: \(message)"
        }
    }
}

struct EmptyResponse: Decodable {}

final class SessionManager {
    static let shared = SessionManager()

    private let keychain: KeychainManager
    private let urlSession: URLSession
    private let refreshLock = NSLock()
    private var refreshTask: Task<Bool, Never>?

    private var baseURL: String {
        APIConfiguration.shared.baseURL
    }

    init(
        keychain: KeychainManager = .shared,
        urlSession: URLSession = .shared
    ) {
        self.keychain = keychain
        self.urlSession = urlSession
    }

    func currentAccessToken() -> String? {
        keychain.retrieveString(for: "hamrah_access_token")
    }

    func hasStoredSession() -> Bool {
        currentAccessToken() != nil || keychain.retrieveString(for: "hamrah_refresh_token") != nil
    }

    func accessTokenForRequest() async throws -> String {
        if let token = currentAccessToken(), !isTokenExpiringSoon() {
            return token
        }

        guard keychain.retrieveString(for: "hamrah_refresh_token") != nil else {
            throw HamrahAPIError.sessionExpired
        }

        guard await refreshAccessToken() else {
            throw HamrahAPIError.sessionExpired
        }

        guard let token = currentAccessToken() else {
            throw HamrahAPIError.sessionExpired
        }

        return token
    }

    func storeTokens(accessToken: String, refreshToken: String?, expiresIn: Int?) {
        _ = keychain.store(accessToken, for: "hamrah_access_token")
        if let refreshToken {
            _ = keychain.store(refreshToken, for: "hamrah_refresh_token")
        } else {
            _ = keychain.delete(for: "hamrah_refresh_token")
        }
        _ = keychain.store(Date().timeIntervalSince1970, for: "hamrah_auth_timestamp")
        if let expiresIn {
            _ = keychain.store(
                Date().timeIntervalSince1970 + TimeInterval(expiresIn),
                for: "hamrah_token_expires_at"
            )
        } else {
            _ = keychain.delete(for: "hamrah_token_expires_at")
        }
        _ = keychain.store(true, for: "hamrah_is_authenticated")
    }

    func clearSession() {
        _ = keychain.clearAllHamrahData()
        UserDefaults.standard.removeObject(forKey: "hamrah_access_token")
        UserDefaults.standard.removeObject(forKey: "hamrah_refresh_token")
        UserDefaults.standard.removeObject(forKey: "hamrah_is_authenticated")
        UserDefaults.standard.removeObject(forKey: "hamrah_auth_timestamp")
        UserDefaults.standard.removeObject(forKey: "hamrah_token_expires_at")
    }

    func isTokenExpiringSoon() -> Bool {
        let expiresAt =
            keychain.retrieveDouble(for: "hamrah_token_expires_at")
            ?? UserDefaults.standard.double(forKey: "hamrah_token_expires_at")

        guard expiresAt > 0 else { return true }
        return expiresAt < Date().timeIntervalSince1970 + (5 * 60)
    }

    func refreshAccessToken() async -> Bool {
        refreshLock.lock()
        if let refreshTask {
            refreshLock.unlock()
            return await refreshTask.value
        }

        let task = Task { await self.performRefreshAccessToken() }
        refreshTask = task
        refreshLock.unlock()

        let result = await task.value

        refreshLock.lock()
        refreshTask = nil
        refreshLock.unlock()

        return result
    }

    private func performRefreshAccessToken() async -> Bool {
        guard let refreshToken = keychain.retrieveString(for: "hamrah_refresh_token") else {
            return false
        }

        guard let url = URL(string: "\(baseURL)/api/auth/tokens/refresh") else {
            return false
        }

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("hamrah-ios", forHTTPHeaderField: "X-Requested-With")
        request.httpBody = try? JSONSerialization.data(
            withJSONObject: ["refresh_token": refreshToken]
        )

        do {
            let (data, response) = try await urlSession.data(for: request)
            guard let httpResponse = response as? HTTPURLResponse else {
                return false
            }

            guard httpResponse.statusCode == 200 else {
                if httpResponse.statusCode == 401 || httpResponse.statusCode == 403 {
                    clearSession()
                }
                return false
            }

            let tokenResponse = try JSONDecoder().decode(TokenRefreshResponse.self, from: data)
            storeTokens(
                accessToken: tokenResponse.access_token,
                refreshToken: tokenResponse.refresh_token,
                expiresIn: tokenResponse.expires_in
            )
            return true
        } catch {
            return false
        }
    }

    private struct TokenRefreshResponse: Decodable {
        let access_token: String
        let refresh_token: String
        let expires_in: Int
    }
}

final class HamrahAPIClient {
    static let shared = HamrahAPIClient()

    private let sessionManager: SessionManager
    private let urlSession: URLSession
    private let attestationManager: AppAttestationManager

    private var baseURL: String {
        APIConfiguration.shared.baseURL
    }

    init(
        sessionManager: SessionManager = .shared,
        urlSession: URLSession = .shared,
        attestationManager: AppAttestationManager = .shared
    ) {
        self.sessionManager = sessionManager
        self.urlSession = urlSession
        self.attestationManager = attestationManager
    }

    func send<Response: Decodable>(
        _ method: HTTPMethod,
        _ endpoint: String,
        auth: APIAuthRequirement = .required,
        body: Encodable? = nil,
        customBaseURL: String? = nil,
        responseType: Response.Type = Response.self
    ) async throws -> Response {
        try await send(
            method,
            endpoint,
            auth: auth,
            body: body,
            customBaseURL: customBaseURL,
            responseType: responseType,
            retryState: .initial
        )
    }

    func get<Response: Decodable>(
        _ endpoint: String,
        auth: APIAuthRequirement = .required,
        customBaseURL: String? = nil,
        responseType: Response.Type = Response.self
    ) async throws -> Response {
        try await send(
            .GET,
            endpoint,
            auth: auth,
            body: nil,
            customBaseURL: customBaseURL,
            responseType: responseType
        )
    }

    func post<Response: Decodable>(
        _ endpoint: String,
        body: Encodable? = nil,
        auth: APIAuthRequirement = .required,
        customBaseURL: String? = nil,
        responseType: Response.Type = Response.self
    ) async throws -> Response {
        try await send(
            .POST,
            endpoint,
            auth: auth,
            body: body,
            customBaseURL: customBaseURL,
            responseType: responseType
        )
    }

    func put<Response: Decodable>(
        _ endpoint: String,
        body: Encodable? = nil,
        auth: APIAuthRequirement = .required,
        customBaseURL: String? = nil,
        responseType: Response.Type = Response.self
    ) async throws -> Response {
        try await send(
            .PUT,
            endpoint,
            auth: auth,
            body: body,
            customBaseURL: customBaseURL,
            responseType: responseType
        )
    }

    func patch<Response: Decodable>(
        _ endpoint: String,
        body: Encodable? = nil,
        auth: APIAuthRequirement = .required,
        customBaseURL: String? = nil,
        responseType: Response.Type = Response.self
    ) async throws -> Response {
        try await send(
            .PATCH,
            endpoint,
            auth: auth,
            body: body,
            customBaseURL: customBaseURL,
            responseType: responseType
        )
    }

    func delete<Response: Decodable>(
        _ endpoint: String,
        auth: APIAuthRequirement = .required,
        responseType: Response.Type = Response.self
    ) async throws -> Response {
        try await send(.DELETE, endpoint, auth: auth, responseType: responseType)
    }

    func initializeAttestationIfNeeded() async {
        guard let token = try? await sessionManager.accessTokenForRequest() else { return }
        do {
            try await initializeAttestationWithRecovery(accessToken: token)
        } catch {
            print("⚠️ Failed to initialize App Attestation: \(error)")
        }
    }

    func debugAppAttestation() async {
        guard let token = try? await sessionManager.accessTokenForRequest() else { return }
        do {
            try await attestationManager.initializeAttestation(accessToken: token)
            print("✅ Debug: App Attestation initialization succeeded")
        } catch {
            print("❌ Debug: App Attestation initialization failed: \(error)")
        }
    }

    private enum RetryState {
        case initial
        case refreshedToken
        case recoveredAttestation
    }

    private func send<Response: Decodable>(
        _ method: HTTPMethod,
        _ endpoint: String,
        auth: APIAuthRequirement,
        body: Encodable?,
        customBaseURL: String?,
        responseType: Response.Type,
        retryState: RetryState
    ) async throws -> Response {
        let requestBody = try encodeBody(body)
        let targetBaseURL = customBaseURL ?? baseURL
        guard let url = buildURL(baseURL: targetBaseURL, endpoint: endpoint) else {
            throw HamrahAPIError.invalidRequest("Invalid URL for \(endpoint)")
        }

        var request = URLRequest(url: url)
        request.httpMethod = method.rawValue
        if requestBody != nil {
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        }
        request.setValue(UUID().uuidString.lowercased(), forHTTPHeaderField: "X-Trace-Id")

        let accessToken = try await token(for: auth)
        if let accessToken {
            request.setValue("Bearer \(accessToken)", forHTTPHeaderField: "Authorization")
            if let userId = decodeJWTSubject(from: accessToken) {
                request.setValue(userId, forHTTPHeaderField: "X-User-Id")
            }
        }

        request.httpBody = requestBody

        let challenge = try generateRequestChallenge(url: url, method: method, body: requestBody)
        try await applyAttestationHeaders(
            to: &request,
            challenge: challenge,
            accessToken: accessToken,
            auth: auth
        )
        request.setValue(challenge.base64EncodedString(), forHTTPHeaderField: "X-Request-Challenge")

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await urlSession.data(for: request)
        } catch {
            throw HamrahAPIError.network(error.localizedDescription)
        }

        guard let httpResponse = response as? HTTPURLResponse else {
            throw HamrahAPIError.network("Invalid response from server")
        }

        if httpResponse.statusCode == 401 {
            let fallbackError = Self.unauthorizedResponseError(data: data)
            return try await handleUnauthorized(
                method,
                endpoint,
                auth: auth,
                body: body,
                customBaseURL: customBaseURL,
                responseType: responseType,
                retryState: retryState,
                fallbackError: fallbackError
            )
        }

        if httpResponse.statusCode == 403 {
            if auth == .required || auth == .bootstrap {
                sessionManager.clearSession()
                throw HamrahAPIError.sessionExpired
            }
            throw HamrahAPIError.unauthorized
        }

        guard (200..<300).contains(httpResponse.statusCode) else {
            throw serverError(statusCode: httpResponse.statusCode, data: data)
        }

        if Response.self == EmptyResponse.self {
            return EmptyResponse() as! Response
        }

        do {
            return try JSONDecoder().decode(Response.self, from: data)
        } catch {
            throw HamrahAPIError.decoding(error.localizedDescription)
        }
    }

    private func handleUnauthorized<Response: Decodable>(
        _ method: HTTPMethod,
        _ endpoint: String,
        auth: APIAuthRequirement,
        body: Encodable?,
        customBaseURL: String?,
        responseType: Response.Type,
        retryState: RetryState,
        fallbackError: HamrahAPIError
    ) async throws -> Response {
        switch (auth, retryState) {
        case (.required, .initial):
            guard await sessionManager.refreshAccessToken() else {
                throw HamrahAPIError.sessionExpired
            }
            return try await send(
                method,
                endpoint,
                auth: auth,
                body: body,
                customBaseURL: customBaseURL,
                responseType: responseType,
                retryState: .refreshedToken
            )
        case (.required, .refreshedToken), (.bootstrap, .initial):
            guard let token = sessionManager.currentAccessToken() else {
                throw HamrahAPIError.sessionExpired
            }
            attestationManager.clearAttestationFlag()
            try await initializeAttestationWithRecovery(accessToken: token)
            return try await send(
                method,
                endpoint,
                auth: auth,
                body: body,
                customBaseURL: customBaseURL,
                responseType: responseType,
                retryState: .recoveredAttestation
            )
        default:
            throw fallbackError
        }
    }

    private func token(for auth: APIAuthRequirement) async throws -> String? {
        switch auth {
        case .none:
            return nil
        case .optional:
            return try? await sessionManager.accessTokenForRequest()
        case .required, .bootstrap:
            return try await sessionManager.accessTokenForRequest()
        }
    }

    private func buildURL(baseURL: String, endpoint: String) -> URL? {
        if let absoluteURL = URL(string: endpoint), absoluteURL.scheme != nil {
            return absoluteURL
        }

        let trimmedBase = baseURL.hasSuffix("/") ? String(baseURL.dropLast()) : baseURL
        let trimmedEndpoint = endpoint.hasPrefix("/") ? String(endpoint.dropFirst()) : endpoint
        return URL(string: "\(trimmedBase)/\(trimmedEndpoint)")
    }

    private func encodeBody(_ body: Encodable?) throws -> Data? {
        guard let body else { return nil }

        if let data = body as? Data {
            return data
        }

        do {
            return try JSONEncoder.iso8601.encode(AnyEncodable(body))
        } catch {
            throw HamrahAPIError.invalidRequest(error.localizedDescription)
        }
    }

    private func applyAttestationHeaders(
        to request: inout URLRequest,
        challenge: Data,
        accessToken: String?,
        auth: APIAuthRequirement
    ) async throws {
        guard auth != .none else {
            setFallbackAttestationHeaders(on: &request)
            return
        }

        do {
            let headers = try await attestationManager.generateAttestationHeaders(for: challenge)
            for (key, value) in headers {
                request.setValue(value, forHTTPHeaderField: key)
            }
        } catch {
            switch Self.attestationFailureStrategy(for: error, accessToken: accessToken) {
            case .fallback:
                setFallbackAttestationHeaders(on: &request)
            case .recoverThenFallback:
                guard let accessToken else {
                    setFallbackAttestationHeaders(on: &request)
                    return
                }
                attestationManager.forceReset()
                do {
                    try await initializeAttestationWithRecovery(accessToken: accessToken)
                    let headers = try await attestationManager.generateAttestationHeaders(
                        for: challenge)
                    for (key, value) in headers {
                        request.setValue(value, forHTTPHeaderField: key)
                    }
                } catch {
                    setFallbackAttestationHeaders(on: &request)
                }
            }
        }
    }

    private func setFallbackAttestationHeaders(on request: inout URLRequest) {
        for (key, value) in Self.fallbackAttestationHeaders(
            bundleIdentifier: Bundle.main.bundleIdentifier ?? "app.hamrah.ios",
            appVersion: Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String
                ?? "unknown"
        ) {
            request.setValue(value, forHTTPHeaderField: key)
        }
    }

    private func initializeAttestationWithRecovery(accessToken: String) async throws {
        do {
            try await attestationManager.initializeAttestation(accessToken: accessToken)
        } catch {
            guard Self.isRecoverableAttestationError(error) else {
                throw HamrahAPIError.attestation(error.localizedDescription)
            }
            attestationManager.forceReset()
            try await attestationManager.initializeAttestation(accessToken: accessToken)
        }
    }

    private func generateRequestChallenge(url: URL, method: HTTPMethod, body: Data?) throws -> Data {
        try Self.requestChallengeData(url: url, method: method, body: body)
    }

    static func requestChallengeData(
        url: URL,
        method: HTTPMethod,
        body: Data?,
        issuedAt: Date = Date(),
        challenge: String = UUID().uuidString.lowercased()
    ) throws -> Data {
        let bodyHash = body.map { SHA256.hash(data: $0).map { String(format: "%02x", $0) }.joined() }
        let clientData = AppAttestRequestClientData(
            challenge: challenge,
            method: method.rawValue,
            url: url.absoluteString,
            bodySha256: bodyHash,
            issuedAt: issuedAt.timeIntervalSince1970
        )
        return try JSONEncoder.iso8601.encode(clientData)
    }

    private func serverError(statusCode: Int, data: Data) -> HamrahAPIError {
        if let errorData = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let errorMessage = errorData["error"] as? String
        {
            return .server(statusCode, errorMessage)
        }
        return .server(statusCode, "Request failed")
    }

    static func unauthorizedResponseError(data: Data) -> HamrahAPIError {
        guard
            let errorData = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let errorMessage = errorData["error"] as? String
        else {
            return .unauthorized
        }

        if errorMessage.localizedCaseInsensitiveContains("attest")
            || errorMessage.localizedCaseInsensitiveContains("attestation")
        {
            return .attestation(errorMessage)
        }

        return .unauthorized
    }

    private func decodeJWTSubject(from token: String) -> String? {
        let parts = token.split(separator: ".")
        guard parts.count >= 2,
            let payloadData = base64URLDecode(String(parts[1])),
            let json = try? JSONSerialization.jsonObject(with: payloadData) as? [String: Any]
        else { return nil }
        return json["sub"] as? String
    }

    private func base64URLDecode(_ input: String) -> Data? {
        var base64 = input.replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        let padding = 4 - (base64.count % 4)
        if padding < 4 {
            base64.append(String(repeating: "=", count: padding))
        }
        return Data(base64Encoded: base64)
    }

    enum AttestationFailureStrategy: Equatable {
        case fallback
        case recoverThenFallback
    }

    static func attestationFailureStrategy(
        for error: Error,
        accessToken: String?
    ) -> AttestationFailureStrategy {
        if accessToken != nil, isRecoverableAttestationError(error) {
            return .recoverThenFallback
        }
        return .fallback
    }

    static func fallbackAttestationHeaders(
        bundleIdentifier: String,
        appVersion: String
    ) -> [String: String] {
        [
            "X-App-Attestation-Mode": "none",
            "X-iOS-Bundle-ID": bundleIdentifier,
            "X-iOS-App-Version": appVersion,
        ]
    }

    static func isRecoverableAttestationError(_ error: Error) -> Bool {
        #if os(iOS)
            if let dcError = error as? DCError {
                return dcError.code == .invalidInput || dcError.code == .invalidKey
                    || dcError.code == .unknownSystemFailure
            }

            let nsError = error as NSError
            if nsError.domain == DCErrorDomain {
                return nsError.code == DCError.Code.invalidInput.rawValue
                    || nsError.code == DCError.Code.invalidKey.rawValue
                    || nsError.code == DCError.Code.unknownSystemFailure.rawValue
            }

            if case AttestationError.keyGenerationFailed = error {
                return true
            }
        #endif

        return false
    }
}

struct AppAttestRequestClientData: Codable, Equatable {
    let challenge: String
    let method: String
    let url: String
    let bodySha256: String?
    let issuedAt: TimeInterval

    enum CodingKeys: String, CodingKey {
        case challenge
        case method
        case url
        case bodySha256 = "body_sha256"
        case issuedAt = "issued_at"
    }
}

struct AnyEncodable: Encodable {
    private let encodeValue: (Encoder) throws -> Void

    init(_ value: Encodable) {
        self.encodeValue = value.encode(to:)
    }

    func encode(to encoder: Encoder) throws {
        try encodeValue(encoder)
    }
}
