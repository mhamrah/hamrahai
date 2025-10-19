//
//  NativeAuthManager.swift
//  hamrahIOS
//
//  Native authentication manager supporting Apple Sign-In, Google Sign-In, and Passkeys
//  Integrates with hamrah.app backend for user management
//

import AuthenticationServices
import Combine
import Foundation
import SwiftUI

#if os(iOS)
    import UIKit
#endif
#if os(macOS)
    import AppKit
#endif

#if canImport(GoogleSignIn) && (os(iOS) || targetEnvironment(macCatalyst))
    import GoogleSignIn
#else
    // Google Sign-In SDK unavailable: build stubs so the rest of the app compiles.
    // These stubs intentionally throw if actually invoked.
    private enum GoogleSignInUnavailableError {
        static func error() -> NSError {
            NSError(
                domain: "GoogleSignInUnavailable",
                code: -1,
                userInfo: [
                    NSLocalizedDescriptionKey: "Google Sign-In SDK is not integrated in this build."
                ]
            )
        }
    }

    class GIDConfiguration {
        init(clientID: String) {}
    }

    class GIDGoogleUser {
        var userID: String? = nil
        var profile: Profile? = Profile()
        var idToken: IDToken? = nil

        class Profile {
            var email: String? = nil
            var name: String? = nil
            func imageURL(withDimension: UInt) -> URL? { nil }
        }

        class IDToken {
            var tokenString: String? = nil
        }
    }

    struct GIDSignInResult {
        let user: GIDGoogleUser
    }

    class GIDSignIn {
        static let sharedInstance = GIDSignIn()
        var configuration: GIDConfiguration?

        func signIn(withPresenting presentingViewController: Any) async throws -> GIDSignInResult {
            throw GoogleSignInUnavailableError.error()
        }
    }
#endif

@MainActor
class NativeAuthManager: NSObject, ObservableObject {
    @Published var isAuthenticated = false
    @Published var currentUser: HamrahUser?
    @Published var isLoading = false
    @Published var errorMessage: String?

    var baseURL: String {
        APIConfiguration.shared.baseURL
    }

    @Published var accessToken: String?

    // Secure API service with App Attestation
    private let secureAPI = SecureAPIService.shared

    struct HamrahUser: Codable {
        let id: String
        let email: String
        let name: String?
        let picture: String?
        let authMethod: String
        let createdAt: String?

        enum CodingKeys: String, CodingKey {
            case id
            case email
            case name
            case picture
            case authMethod = "auth_method"
            case createdAt = "created_at"
        }

        // Explicit memberwise initializer
        init(
            id: String, email: String, name: String?, picture: String?, authMethod: String,
            createdAt: String?
        ) {
            self.id = id
            self.email = email
            self.name = name
            self.picture = picture
            self.authMethod = authMethod
            self.createdAt = createdAt
        }
    }

    struct AuthResponse: Codable {
        let success: Bool
        let user: HamrahUser?
        let accessToken: String?
        let refreshToken: String?
        let expiresIn: Int?
        let error: String?

        // Handle different possible field names for access token
        enum CodingKeys: String, CodingKey {
            case success
            case user
            case accessToken = "access_token"
            case refreshToken = "refresh_token"
            case expiresIn = "expires_in"
            case error
        }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)

            // Decode tokens and optional fields first
            let accessTokenDecoded = try container.decodeIfPresent(
                String.self, forKey: .accessToken)
            let refreshTokenDecoded = try container.decodeIfPresent(
                String.self, forKey: .refreshToken)
            let expiresInDecoded = try container.decodeIfPresent(Int.self, forKey: .expiresIn)
            let decodedUser = try container.decodeIfPresent(HamrahUser.self, forKey: .user)
            let errorDecoded = try container.decodeIfPresent(String.self, forKey: .error)

            // Derive user from JWT claims if not provided
            var userDerived = decodedUser
            if userDerived == nil, let token = accessTokenDecoded,
                let claims = AuthResponse.decodeJWTClaims(token)
            {
                let email = claims["email"] as? String
                let id = (claims["sub"] as? String) ?? UUID().uuidString
                let name = claims["name"] as? String
                if let email = email {
                    userDerived = HamrahUser(
                        id: id,
                        email: email,
                        name: name,
                        picture: nil,
                        authMethod: "google",
                        createdAt: nil
                    )
                }
            }

            // Default success to true if we received tokens, otherwise decode explicit success
            let decodedSuccess = try container.decodeIfPresent(Bool.self, forKey: .success)
            let successDerived = decodedSuccess ?? (accessTokenDecoded != nil)

            // Assign
            success = successDerived
            user = userDerived
            accessToken = accessTokenDecoded
            refreshToken = refreshTokenDecoded
            expiresIn = expiresInDecoded
            error = errorDecoded
        }

        func encode(to encoder: Encoder) throws {
            var container = encoder.container(keyedBy: CodingKeys.self)
            try container.encode(success, forKey: .success)
            try container.encodeIfPresent(user, forKey: .user)
            try container.encodeIfPresent(accessToken, forKey: .accessToken)
            try container.encodeIfPresent(refreshToken, forKey: .refreshToken)
            try container.encodeIfPresent(expiresIn, forKey: .expiresIn)
            try container.encodeIfPresent(error, forKey: .error)
        }

        // Decode JWT payload claims (Base64URL) into a dictionary
        private static func decodeJWTClaims(_ jwt: String) -> [String: Any]? {
            let segments = jwt.split(separator: ".")
            guard segments.count >= 2 else { return nil }
            let payload = String(segments[1])
            var base64 =
                payload
                .replacingOccurrences(of: "-", with: "+")
                .replacingOccurrences(of: "_", with: "/")
            let padding = 4 - (base64.count % 4)
            if padding < 4 {
                base64.append(String(repeating: "=", count: padding))
            }
            guard let data = Data(base64Encoded: base64) else { return nil }
            do {
                let obj = try JSONSerialization.jsonObject(with: data, options: [])
                return obj as? [String: Any]
            } catch {
                return nil
            }
        }
    }

    struct WebAuthnBeginResponse: Codable {
        let success: Bool
        let options: PublicKeyCredentialRequestOptions?
        let challengeId: String
        let error: String?

        enum CodingKeys: String, CodingKey {
            case success
            case options
            case challengeId = "challenge_id"
            case error
        }
    }

    struct PublicKeyCredentialRequestOptions: Codable {
        let challenge: String
        let timeout: Int?
        let rpId: String
        let allowCredentials: [PublicKeyCredentialDescriptor]?
        let userVerification: String?
        let challengeId: String

        enum CodingKeys: String, CodingKey {
            case challenge
            case timeout
            case rpId = "rp_id"
            case allowCredentials = "allow_credentials"
            case userVerification = "user_verification"
            case challengeId = "challenge_id"
        }
    }

    struct PublicKeyCredentialDescriptor: Codable {
        let type: String
        let id: String
        let transports: [String]?

        enum CodingKeys: String, CodingKey {
            case type
            case id
            case transports
        }
    }

    override init() {
        super.init()
        loadStoredAuth()
        configureGoogleSignIn()
    }

    // Test-specific initializer that ensures production environment
    static func testInstance() -> NativeAuthManager {
        // Force production environment for tests
        APIConfiguration.shared.currentEnvironment = .production
        APIConfiguration.shared.customBaseURL = ""

        let manager = NativeAuthManager()
        return manager
    }

    // MARK: - Apple Sign-In

    func signInWithApple() async {
        isLoading = true
        errorMessage = nil

        print("🍎 Starting Apple Sign-In...")

        let request = ASAuthorizationAppleIDProvider().createRequest()
        request.requestedScopes = [.fullName, .email]

        let authController = ASAuthorizationController(authorizationRequests: [request])
        authController.delegate = self
        authController.presentationContextProvider = self

        authController.performRequests()
    }

    // MARK: - Google Sign-In

    private func configureGoogleSignIn() {
        // Prefer GoogleService-Info.plist, but fall back to Info.plist (GIDClientID)
        var clientId: String? = nil
        if let path = Bundle.main.path(forResource: "GoogleService-Info", ofType: "plist"),
            let plist = NSDictionary(contentsOfFile: path),
            let id = plist["CLIENT_ID"] as? String,
            !id.isEmpty
        {
            clientId = id
            print("✅ Google Sign-In configured from GoogleService-Info.plist")
        } else if let id = Bundle.main.object(forInfoDictionaryKey: "GIDClientID") as? String,
            !id.isEmpty
        {
            clientId = id
            print("✅ Google Sign-In configured from Info.plist GIDClientID")
        } else {
            print("⚠️ GoogleService-Info.plist not found and GIDClientID missing in Info.plist")
        }
        if let clientId = clientId {
            GIDSignIn.sharedInstance.configuration = GIDConfiguration(clientID: clientId)
            print("✅ Google Sign-In configured")
        }
    }

    func signInWithGoogle() async {
        isLoading = true
        errorMessage = nil
        do {
            print("🔍 Starting Google Sign-In...")
            #if os(iOS) || targetEnvironment(macCatalyst)
                // Ensure app is active before presenting Google Sign-In
                if UIApplication.shared.applicationState != .active {
                    print("⏳ Waiting for app to become active before starting Google Sign-In")
                    while UIApplication.shared.applicationState != .active {
                        try await Task.sleep(nanoseconds: 100_000_000)
                    }
                }
                guard
                    let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
                    let window = windowScene.windows.first(where: { $0.isKeyWindow }),
                    var presentingViewController = window.rootViewController
                else {
                    throw NSError(
                        domain: "GoogleSignIn",
                        code: -1,
                        userInfo: [NSLocalizedDescriptionKey: "No presenting view controller"]
                    )
                }
                // Traverse to the top-most presented view controller
                while let presented = presentingViewController.presentedViewController {
                    presentingViewController = presented
                }
                let result = try await GIDSignIn.sharedInstance.signIn(
                    withPresenting: presentingViewController)
                let user = result.user
                print("🔍 Google Sign-In result received:")
                print("  User ID: \(user.userID ?? "nil")")
                print("  Email: \(user.profile?.email ?? "nil")")
                print("  Name: \(user.profile?.name ?? "nil")")
                guard let idToken = user.idToken?.tokenString else {
                    throw NSError(
                        domain: "GoogleSignIn",
                        code: -1,
                        userInfo: [NSLocalizedDescriptionKey: "No ID token"]
                    )
                }
                print("🔍 Google ID token received, length: \(idToken.count)")
                print("🔍 Sending authentication request to backend...")
                try await authenticateWithBackend(
                    provider: "google",
                    credential: idToken,
                    additionalData: [
                        "email": user.profile?.email ?? "",
                        "name": user.profile?.name ?? "",
                        "picture": user.profile?.imageURL(withDimension: 200)?.absoluteString ?? "",
                    ]
                )
                print("🔍 Google backend authentication completed successfully")
            #else
                throw NSError(
                    domain: "GoogleSignIn",
                    code: -2,
                    userInfo: [NSLocalizedDescriptionKey: "Unsupported platform for Google Sign-In"]
                )
            #endif
        } catch {
            errorMessage = "Google Sign-In failed: \(error.localizedDescription)"
            print("❌ Google Sign-In error: \(error)")
        }
        isLoading = false
    }

    // MARK: - Passkey Authentication

    func checkPasskeyAvailability() async -> Bool {
        guard let token = accessToken else {
            print("🔍 No access token available for passkey check")
            return false
        }

        guard let userId = currentUser?.id else {
            print("🔍 No user ID available for passkey check")
            return false
        }

        do {
            struct PasskeyCredentialsResponse: Codable {
                let success: Bool
                let credentials: [PasskeyCredentialInfo]
                let error: String?
            }

            struct PasskeyCredentialInfo: Codable {
                let id: String
                let name: String?

                enum CodingKeys: String, CodingKey {
                    case id
                    case name
                }
            }

            let response = try await secureAPI.get(
                endpoint: "/api/webauthn/users/\(userId)/credentials",
                accessToken: token,
                responseType: PasskeyCredentialsResponse.self,
                customBaseURL: nil
            )

            let hasPasskeys = response.success && !response.credentials.isEmpty
            print(
                "🔍 Passkey availability check: \(hasPasskeys ? "has passkeys (\(response.credentials.count))" : "no passkeys")"
            )
            return hasPasskeys

        } catch {
            print("🔍 Passkey availability check error: \(error)")
            return false
        }
    }

    func signInWithPasskey(email: String) async {
        isLoading = true
        errorMessage = nil

        do {
            print("🔐 Starting Passkey authentication for \(email)...")

            // Step 1: Begin WebAuthn authentication
            let beginOptions = try await beginWebAuthnAuthentication(email: email)

            guard let options = beginOptions.options else {
                throw NSError(
                    domain: "WebAuthn", code: -1,
                    userInfo: [NSLocalizedDescriptionKey: "No authentication options received"])
            }

            let challengeId = beginOptions.challengeId

            // Step 2: Perform platform authentication
            let assertion = try await performPlatformAuthentication(options: options)

            // Step 3: Complete authentication with backend
            try await completeWebAuthnAuthentication(assertion: assertion, challengeId: challengeId)

        } catch {
            errorMessage = "Passkey authentication failed: \(error.localizedDescription)"
            print("❌ Passkey error: \(error)")
        }

        isLoading = false
    }

    private func beginWebAuthnAuthentication(email: String?) async throws -> WebAuthnBeginResponse {
        var body: [String: Any] = ["explicit": true]  // flag for explicit discoverable auth
        if let email = email {
            body["email"] = email
        }

        return try await secureAPI.post(
            endpoint: "/api/webauthn/authenticate/discoverable",
            body: body,
            accessToken: nil,  // No auth needed for discoverable begin
            responseType: WebAuthnBeginResponse.self,
            customBaseURL: baseURL
        )
    }

    private func performPlatformAuthentication(options: PublicKeyCredentialRequestOptions)
        async throws -> ASAuthorizationPlatformPublicKeyCredentialAssertion
    {
        let challenge = Data(base64Encoded: options.challenge) ?? Data()

        let request = ASAuthorizationPlatformPublicKeyCredentialProvider(
            relyingPartyIdentifier: options.rpId
        )
        .createCredentialAssertionRequest(challenge: challenge)

        return try await withCheckedThrowingContinuation { continuation in
            let controller = ASAuthorizationController(authorizationRequests: [request])

            // Store continuation for delegate callback
            PasskeyAuthDelegate.shared.setContinuation(continuation)

            controller.delegate = PasskeyAuthDelegate.shared
            controller.presentationContextProvider = self
            controller.performRequests()
        }
    }

    private func completeWebAuthnAuthentication(
        assertion: ASAuthorizationPlatformPublicKeyCredentialAssertion, challengeId: String
    ) async throws {
        // Build SimpleWebAuthn-style assertion payload
        let authResponseData =
            [
                "id": assertion.credentialID.base64EncodedString(),
                "rawId": assertion.credentialID.base64EncodedString(),
                "type": "public-key",
                "response": [
                    "authenticatorData": assertion.rawAuthenticatorData.base64EncodedString(),
                    "clientDataJSON": assertion.rawClientDataJSON.base64EncodedString(),
                    "signature": assertion.signature.base64EncodedString(),
                    "userHandle": assertion.userID?.base64EncodedString() ?? "",
                ],
            ] as [String: Any]

        let body =
            [
                "response": authResponseData,
                "challenge_id": challengeId,
                "mode": "discoverable-explicit",
            ] as [String: Any]

        struct PasskeyAuthResponse: Codable {
            let success: Bool
            let user: HamrahUser?
            let session_token: String?
            let error: String?

            enum CodingKeys: String, CodingKey {
                case success
                case user
                case session_token
                case error
            }
        }

        // Directly call the new verify endpoint (no cookie extraction required; session token returned in body)
        let result = try await secureAPI.post(
            endpoint: "/api/webauthn/authenticate/discoverable/verify",
            body: body,
            accessToken: nil,
            responseType: PasskeyAuthResponse.self,
            customBaseURL: baseURL
        )

        guard result.success, let user = result.user, let token = result.session_token else {
            throw NSError(
                domain: "WebAuthn", code: -1,
                userInfo: [NSLocalizedDescriptionKey: result.error ?? "Authentication failed"])
        }

        self.currentUser = user
        self.accessToken = token
        self.isAuthenticated = true
        self.setLastUsedEmail(user.email)
        self.storeAuthState()

        // Initialize App Attestation (blocks on first install, skips if already initialized)
        await secureAPI.initializeAttestation(accessToken: token)

        print("✅ Passkey authentication successful")
    }

    // MARK: - Session Token Extraction

    private func extractSessionToken(from setCookieHeader: String) -> String? {
        // Look for the session token cookie in the Set-Cookie header
        // Format: "session=token_value; Path=/; HttpOnly; Secure; SameSite=Lax"
        let components = setCookieHeader.components(separatedBy: ";")
        for component in components {
            let cookiePart = component.trimmingCharacters(in: .whitespaces)
            if cookiePart.hasPrefix("session=") {
                return String(cookiePart.dropFirst("session=".count))
            }
        }
        return nil
    }

    // MARK: - Backend Integration

    private func authenticateWithBackend(
        provider: String, credential: String, additionalData: [String: String] = [:]
    ) async throws {
        let url = URL(string: "\(baseURL)/api/auth/native")!
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("hamrah-ios", forHTTPHeaderField: "X-Requested-With")

        var body = [
            "provider": provider,
            "credential": credential,
        ]

        // Add additional data
        for (key, value) in additionalData {
            body[key] = value
        }

        request.httpBody = try JSONEncoder().encode(body)

        let (data, response) = try await URLSession.shared.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse,
            httpResponse.statusCode == 200
        else {
            let statusCode = (response as? HTTPURLResponse)?.statusCode ?? -1
            print("❌ Auth failed with status code: \(statusCode)")
            throw NSError(
                domain: "Auth", code: -1,
                userInfo: [
                    NSLocalizedDescriptionKey:
                        "Backend authentication failed with status \(statusCode)"
                ])
        }

        let authResponse: AuthResponse
        do {
            authResponse = try JSONDecoder().decode(AuthResponse.self, from: data)
        } catch {
            print("❌ JSON Decoding Error: \(error)")
            print("❌ Raw response: \(String(data: data, encoding: .utf8) ?? "nil")")
            throw NSError(
                domain: "Auth", code: -1,
                userInfo: [
                    NSLocalizedDescriptionKey:
                        "Failed to parse server response: \(error.localizedDescription)"
                ])
        }

        if let token = authResponse.accessToken {
            await MainActor.run {
                self.accessToken = token
                self.isAuthenticated = true

                if let user = authResponse.user {
                    self.currentUser = user
                    // Store the user's email for future automatic login
                    self.setLastUsedEmail(user.email)
                } else {
                    // Fallback: derive a minimal user from the data we have
                    let email = additionalData["email"] ?? self.currentUser?.email ?? ""
                    let name = additionalData["name"]
                    let picture = additionalData["picture"]
                    let id = self.currentUser?.id ?? UUID().uuidString
                    self.currentUser = HamrahUser(
                        id: id,
                        email: email,
                        name: name,
                        picture: picture,
                        authMethod: provider,
                        createdAt: nil
                    )
                    if !email.isEmpty {
                        self.setLastUsedEmail(email)
                    }
                }
            }

            // Store refresh token if provided
            if let refreshToken = authResponse.refreshToken {
                _ = KeychainManager.shared.store(refreshToken, for: "hamrah_refresh_token")
            }

            // Store token expiration if provided
            if let expiresIn = authResponse.expiresIn {
                let expiresAt = Date().timeIntervalSince1970 + TimeInterval(expiresIn)
                _ = KeychainManager.shared.store(expiresAt, for: "hamrah_token_expires_at")
            }

            self.storeAuthState()

            // Initialize App Attestation (blocks on first install, skips if already initialized)
            if let token = self.accessToken {
                await secureAPI.initializeAttestation(accessToken: token)
            }

            let emailLog = self.currentUser?.email ?? "(unknown)"
            print("✅ Backend authentication successful - Token accepted for \(emailLog)")
        } else {
            print("❌ Auth Response Validation Failed:")
            print("  Success: \(authResponse.success)")
            print("  User nil: \(authResponse.user == nil)")
            print("  Token nil: \(authResponse.accessToken == nil)")
            print("  Error: \(authResponse.error ?? "none")")

            throw NSError(
                domain: "Auth", code: -1,
                userInfo: [
                    NSLocalizedDescriptionKey: authResponse.error
                        ?? "Authentication failed - invalid response format"
                ])
        }
    }

    // MARK: - Token Validation

    func validateAccessToken() async -> Bool {
        guard let token = accessToken else { return false }

        guard currentUser?.id != nil else {
            print("🔍 No user ID available for token validation")
            return false
        }

        // Try to validate with a backend endpoint that we know exists
        let url = URL(string: "\(baseURL)/api/auth/tokens/validate")!
        var request = URLRequest(url: url)
        request.httpMethod = "GET"
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.setValue("hamrah-ios", forHTTPHeaderField: "X-Requested-With")

        do {
            let (_, response) = try await URLSession.shared.data(for: request)

            guard let httpResponse = response as? HTTPURLResponse else { return false }

            if httpResponse.statusCode == 200 {
                // Token is valid
                print("🔍 Token validation successful")
                return true
            } else if httpResponse.statusCode == 401 {
                // Token is invalid/expired
                print("🔍 Token validation failed: token expired (401)")
                await MainActor.run {
                    logout()
                }
                return false
            } else {
                // Other errors - assume token is still valid but endpoint might have issues
                print(
                    "🔍 Token validation inconclusive with status: \(httpResponse.statusCode), assuming valid"
                )
                return true
            }
        } catch {
            print("🔍 Token validation error: \(error), assuming valid")
            // If we can't validate due to network issues, assume token is valid
            return true
        }
    }

    // MARK: - Token Refresh

    func refreshToken() async -> Bool {
        guard let refreshToken = KeychainManager.shared.retrieveString(for: "hamrah_refresh_token")
        else {
            print("🔄 No refresh token available")
            return false
        }

        let url = URL(string: "\(baseURL)/api/auth/tokens/refresh")!
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("hamrah-ios", forHTTPHeaderField: "X-Requested-With")

        let body = ["refresh_token": refreshToken]

        do {
            request.httpBody = try JSONSerialization.data(withJSONObject: body)
            let (data, response) = try await URLSession.shared.data(for: request)

            guard let httpResponse = response as? HTTPURLResponse,
                httpResponse.statusCode == 200
            else {
                print(
                    "🔄 Token refresh failed with status: \((response as? HTTPURLResponse)?.statusCode ?? -1)"
                )
                return false
            }

            struct TokenRefreshResponse: Codable {
                let access_token: String
                let refresh_token: String
                let expires_in: Int

                enum CodingKeys: String, CodingKey {
                    case access_token
                    case refresh_token
                    case expires_in
                }
            }

            let tokenResponse = try JSONDecoder().decode(TokenRefreshResponse.self, from: data)

            // Update stored tokens
            await MainActor.run {
                self.accessToken = tokenResponse.access_token
                _ = KeychainManager.shared.store(
                    tokenResponse.access_token, for: "hamrah_access_token")
                _ = KeychainManager.shared.store(
                    tokenResponse.refresh_token, for: "hamrah_refresh_token")
                _ = KeychainManager.shared.store(
                    Date().timeIntervalSince1970, for: "hamrah_auth_timestamp")
                _ = KeychainManager.shared.store(
                    Date().timeIntervalSince1970 + TimeInterval(tokenResponse.expires_in),
                    for: "hamrah_token_expires_at")
            }

            print("✅ Token refreshed successfully")
            return true

        } catch {
            print("❌ Token refresh error: \(error)")
            return false
        }
    }

    func isTokenExpiringSoon() -> Bool {
        // Check Keychain first (current storage), then UserDefaults (for backward compatibility/tests)
        let expiresAt =
            KeychainManager.shared.retrieveDouble(for: "hamrah_token_expires_at")
            ?? UserDefaults.standard.double(forKey: "hamrah_token_expires_at")

        guard expiresAt > 0 else { return true }

        let fiveMinutesFromNow = Date().timeIntervalSince1970 + (5 * 60)  // 5 minutes
        return expiresAt < fiveMinutesFromNow
    }

    // MARK: - Storage

    private func storeAuthState() {
        let keychain = KeychainManager.shared

        // Store user data
        if let user = currentUser, let userData = try? JSONEncoder().encode(user) {
            _ = keychain.store(userData, for: "hamrah_user")
        }

        // Store tokens securely
        if let token = accessToken {
            _ = keychain.store(token, for: "hamrah_access_token")
        }

        // Store authentication state
        _ = keychain.store(isAuthenticated, for: "hamrah_is_authenticated")

        // Store timestamp for token validation
        _ = keychain.store(Date().timeIntervalSince1970, for: "hamrah_auth_timestamp")
    }

    private func loadStoredAuth() {
        let keychain = KeychainManager.shared

        // Migration removed: tokens are stored in Keychain only

        // Load from secure Keychain
        isAuthenticated = keychain.retrieveBool(for: "hamrah_is_authenticated") ?? false
        accessToken = keychain.retrieveString(for: "hamrah_access_token")

        if let userData = keychain.retrieve(for: "hamrah_user"),
            let user = try? JSONDecoder().decode(HamrahUser.self, from: userData)
        {
            currentUser = user
        }

        // Check if token is stale (older than 24 hours)
        let authTimestamp = keychain.retrieveDouble(for: "hamrah_auth_timestamp") ?? 0
        let dayAgo = Date().timeIntervalSince1970 - (24 * 60 * 60)  // 24 hours

        if authTimestamp > 0 && authTimestamp < dayAgo {
            clearStoredAuth()
            isAuthenticated = false
            currentUser = nil
            accessToken = nil
        }
    }

    private func clearStoredAuth() {
        let keychain = KeychainManager.shared

        // Clear from Keychain
        _ = keychain.clearAllHamrahData()

        // Clear legacy UserDefaults values for backward compatibility
        UserDefaults.standard.removeObject(forKey: "hamrah_access_token")
        UserDefaults.standard.removeObject(forKey: "hamrah_refresh_token")
        UserDefaults.standard.removeObject(forKey: "hamrah_is_authenticated")
        UserDefaults.standard.removeObject(forKey: "hamrah_auth_timestamp")
        UserDefaults.standard.removeObject(forKey: "hamrah_token_expires_at")

        // Don't clear last used email for passkey auto-login
    }

    // Migration from UserDefaults removed; tokens are stored in Keychain only

    // MARK: - Last Used Email for Passkey Auto-Login

    func getLastUsedEmail() -> String? {
        return UserDefaults.standard.string(forKey: "hamrah_last_email")
    }

    func setLastUsedEmail(_ email: String) {
        UserDefaults.standard.set(email, forKey: "hamrah_last_email")
    }

    func clearLastUsedEmail() {
        UserDefaults.standard.removeObject(forKey: "hamrah_last_email")
    }

    // MARK: - Logout

    func logout() {
        isAuthenticated = false
        currentUser = nil
        accessToken = nil
        clearStoredAuth()
        print("🚪 User logged out")
    }

    // MARK: - Authentication State Management

    func loadAuthenticationState() async {
        await MainActor.run {
            loadStoredAuth()
        }
    }

    func hasValidStoredTokens() -> Bool {
        let keychain = KeychainManager.shared
        let hasAccessToken = keychain.retrieveString(for: "hamrah_access_token") != nil
        let hasRefreshToken = keychain.retrieveString(for: "hamrah_refresh_token") != nil
        return hasAccessToken || hasRefreshToken
    }

    func forceReauthentication() {
        print("🔒 Forcing reauthentication - clearing auth state")
        logout()
    }
}

// MARK: - Apple Sign-In Delegate

extension NativeAuthManager: ASAuthorizationControllerDelegate {
    func authorizationController(
        controller: ASAuthorizationController,
        didCompleteWithAuthorization authorization: ASAuthorization
    ) {
        print("🍎 Apple Sign-In authorization completed")
        Task {
            do {
                if let appleIDCredential = authorization.credential
                    as? ASAuthorizationAppleIDCredential
                {
                    print("🍎 Apple ID Credential received:")
                    print("  User ID: \(appleIDCredential.user)")
                    print("  Email: \(appleIDCredential.email ?? "nil")")
                    print("  Full Name: \(appleIDCredential.fullName?.description ?? "nil")")

                    guard let identityToken = appleIDCredential.identityToken,
                        let tokenString = String(data: identityToken, encoding: .utf8)
                    else {
                        throw NSError(
                            domain: "AppleSignIn", code: -1,
                            userInfo: [NSLocalizedDescriptionKey: "No identity token"])
                    }

                    print("🍎 Identity token received, length: \(tokenString.count)")

                    let additionalData = [
                        "email": appleIDCredential.email ?? "",
                        "name": [
                            appleIDCredential.fullName?.givenName,
                            appleIDCredential.fullName?.familyName,
                        ]
                        .compactMap { $0 }
                        .joined(separator: " "),
                    ]

                    print("🍎 Sending authentication request to backend...")
                    try await authenticateWithBackend(
                        provider: "apple", credential: tokenString, additionalData: additionalData)
                    print("🍎 Backend authentication completed successfully")
                } else {
                    print("❌ Apple Sign-In: Invalid credential type")
                }
            } catch {
                await MainActor.run {
                    errorMessage = "Apple Sign-In failed: \(error.localizedDescription)"
                }
                print("❌ Apple Sign-In completion error: \(error)")
            }
            await MainActor.run {
                isLoading = false
            }
            print("🍎 Apple Sign-In flow completed, isLoading set to false")
        }
    }

    func authorizationController(
        controller: ASAuthorizationController, didCompleteWithError error: Error
    ) {
        errorMessage = "Apple Sign-In failed: \(error.localizedDescription)"
        print("❌ Apple Sign-In error: \(error)")
        isLoading = false
    }
}

// MARK: - Presentation Context Provider

extension NativeAuthManager: ASAuthorizationControllerPresentationContextProviding {
    func presentationAnchor(for controller: ASAuthorizationController) -> ASPresentationAnchor {
        #if os(iOS)
            guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
                let window = windowScene.windows.first
            else {
                return ASPresentationAnchor()
            }
            return window
        #elseif os(macOS)
            return NSApplication.shared.windows.first ?? ASPresentationAnchor()
        #else
            return ASPresentationAnchor()
        #endif
    }
}

// MARK: - Passkey Auth Delegate

class PasskeyAuthDelegate: NSObject, ASAuthorizationControllerDelegate {
    static let shared = PasskeyAuthDelegate()

    private var continuation:
        CheckedContinuation<ASAuthorizationPlatformPublicKeyCredentialAssertion, Error>?

    func setContinuation(
        _ continuation: CheckedContinuation<
            ASAuthorizationPlatformPublicKeyCredentialAssertion, Error
        >
    ) {
        self.continuation = continuation
    }

    func authorizationController(
        controller: ASAuthorizationController,
        didCompleteWithAuthorization authorization: ASAuthorization
    ) {
        if let assertion = authorization.credential
            as? ASAuthorizationPlatformPublicKeyCredentialAssertion
        {
            continuation?.resume(returning: assertion)
        } else {
            continuation?.resume(
                throwing: NSError(
                    domain: "PasskeyAuth", code: -1,
                    userInfo: [NSLocalizedDescriptionKey: "Invalid credential type"]))
        }
        continuation = nil
    }

    func authorizationController(
        controller: ASAuthorizationController, didCompleteWithError error: Error
    ) {
        continuation?.resume(throwing: error)
        continuation = nil
    }
}

// MARK: - Passkey Availability Delegate

class PasskeyAvailabilityDelegate: NSObject, ASAuthorizationControllerDelegate {
    private let completion: (Bool) -> Void
    private var hasCompleted = false

    init(completion: @escaping (Bool) -> Void) {
        self.completion = completion
        super.init()
    }

    func timeoutIfNeeded(timeoutCompletion: () -> Void) {
        if !hasCompleted {
            hasCompleted = true
            timeoutCompletion()
        }
    }

    func authorizationController(
        controller: ASAuthorizationController,
        didCompleteWithAuthorization authorization: ASAuthorization
    ) {
        guard !hasCompleted else { return }
        hasCompleted = true
        // If we get here, passkeys are available
        completion(true)
    }

    func authorizationController(
        controller: ASAuthorizationController, didCompleteWithError error: Error
    ) {
        guard !hasCompleted else { return }
        hasCompleted = true

        // Check if error indicates no passkeys are available
        if let asError = error as? ASAuthorizationError {
            switch asError.code {
            case .notHandled, .notInteractive:
                completion(false)
            default:
                completion(true)  // Other errors might still mean passkeys are available
            }
        } else {
            completion(false)
        }
    }
}

// MARK: - Helper Extensions for NativeAuthManager

extension NativeAuthManager {
    private func generateRequestChallenge(url: URL, method: String, body: [String: Any]?) -> Data {
        // Create a deterministic challenge based on request details
        var challengeString = "\(method):\(url.absoluteString)"

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
}
