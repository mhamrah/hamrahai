#if os(iOS)
    import Foundation
    import DeviceCheck
    import CommonCrypto
    import UIKit
    import Combine

    class AppAttestationManager: ObservableObject {
        let objectWillChange = ObservableObjectPublisher()
        static let shared = AppAttestationManager()

        private let service = DCAppAttestService.shared
        private let keychain = KeychainManager.shared
        private let keyId = "hamrah_app_attest_key"
        private var baseURL: String {
            APIConfiguration.shared.baseURL
        }

        private init() {}

        // MARK: - Public Interface

        /// Generates attestation headers for API requests
        func generateAttestationHeaders(for challenge: Data) async throws -> [String: String] {
            #if targetEnvironment(simulator)
                // Simulator: Use development-only identification
                return generateSimulatorHeaders()
            #else
                // Physical device: Use full App Attestation
                return try await generateDeviceAttestationHeaders(for: challenge)
            #endif
        }

        #if targetEnvironment(simulator)
            private func generateSimulatorHeaders() -> [String: String] {
                print("🔧 Using simulator development headers (App Attestation not available)")
                return [
                    "X-iOS-Development": "simulator",
                    "X-iOS-Bundle-ID": Bundle.main.bundleIdentifier ?? "app.hamrah.ios",
                    "X-iOS-Simulator-ID": UIDevice.current.identifierForVendor?.uuidString
                        ?? "unknown",
                    "X-iOS-App-Version": Bundle.main.infoDictionary?["CFBundleShortVersionString"]
                        as? String ?? "unknown",
                ]
            }
        #endif

        private func generateDeviceAttestationHeaders(for challenge: Data) async throws -> [String:
            String]
        {
            let keyId = try await ensureAttestationKey()
            let assertion = try await generateAssertion(keyId: keyId, challenge: challenge)

            return [
                "X-iOS-App-Attest-Key": keyId,
                "X-iOS-App-Attest-Assertion": assertion.base64EncodedString(),
                "X-iOS-App-Bundle-ID": Bundle.main.bundleIdentifier ?? "app.hamrah.ios",
            ]
        }

        /// One-time setup: Generate key and get attestation from Apple
        func initializeAttestation(accessToken: String) async throws {
            #if targetEnvironment(simulator)
                print("🔧 Skipping App Attestation initialization on simulator")
                return
            #else
                print("🔐 Initializing iOS App Attestation...")

                // Step 1: Generate attestation key if needed
                print("  -> Step 1: Ensuring attestation key exists...")
                let keyId = try await ensureAttestationKey()
                print("  -> Step 1: Attestation key ready.")

                // Step 2: Skip one-time attestation if local setup completed.
                // If the server later rejects an assertion, SecureAPIService will recover once.
                if keychain.retrieveString(for: "hamrah_attestation_completed") == "true" {
                    print("✅ App Attestation already initialized locally")
                    return
                }

                // Step 3: Get challenge from server
                let challenge = try await getAttestationChallenge(accessToken: accessToken)

                // Step 4: Generate attestation from Apple
                print("  -> Step 4: Generating attestation from Apple...")
                let attestationData = try await generateAttestation(
                    keyId: keyId, challenge: challenge.challengeData)
                print("  -> Step 4: Attestation generated successfully.")

                // Step 5: Submit attestation to server for verification
                print("  -> Step 5: Submitting attestation to server for verification...")
                try await submitAttestation(
                    attestation: attestationData,
                    keyId: keyId,
                    challengeId: challenge.challengeId,
                    accessToken: accessToken
                )
                print("  -> Step 5: Attestation submitted and verified successfully.")

                // Mark as completed
                _ = keychain.store("true", for: "hamrah_attestation_completed")
                print("✅ iOS App Attestation initialization completed")
            #endif
        }

        // MARK: - Private Implementation

        private func ensureAttestationKey() async throws -> String {
            // Check if we already have a key ID stored
            if let existingKeyId = keychain.retrieveString(for: keyId) {
                print("🔑 Found existing App Attestation key: \(existingKeyId.prefix(8))...")
                return existingKeyId
            }

            // Generate new key - only works on physical devices
            guard service.isSupported else {
                let errorMessage = "App Attestation not supported on this device"
                print("❌ \(errorMessage)")
                throw AttestationError.notSupported
            }

            do {
                print("  -> Calling DCAppAttestService.generateKey()...")
                let newKeyId = try await service.generateKey()
                print("  -> DCAppAttestService.generateKey() succeeded.")

                // Store key ID securely
                guard keychain.store(newKeyId, for: keyId) else {
                    throw AttestationError.keyStorageFailed
                }

                print("🔑 Generated new App Attestation key: \(newKeyId.prefix(8))...")
                return newKeyId
            } catch {
                print("❌ Failed to generate App Attestation key: \(error)")
                if let dcError = error as? DCError {
                    print("❌ DCError code: \(dcError.code.rawValue)")
                    print("❌ DCError description: \(dcError.localizedDescription)")
                }
                throw AttestationError.keyGenerationFailed(error.localizedDescription)
            }
        }

        private func generateAttestation(keyId: String, challenge: Data) async throws -> Data {
            // Validate inputs
            print("🔍 Validating attestation inputs...")
            print("  -> Key ID: \(keyId.prefix(8))... (length: \(keyId.count))")
            print("  -> Challenge data length: \(challenge.count) bytes")

            // Verify the key still exists in keychain
            guard let storedKeyId = keychain.retrieveString(for: self.keyId), storedKeyId == keyId
            else {
                print("❌ Key ID mismatch or missing from keychain")
                print("  -> Expected: \(keyId.prefix(8))...")
                print(
                    "  -> Stored: \(keychain.retrieveString(for: self.keyId)?.prefix(8) ?? "nil")..."
                )
                throw AttestationError.keyGenerationFailed("Key ID not found in keychain")
            }

            // Create hash of challenge as required by App Attest
            let challengeHash = sha256(challenge)
            print(
                "  -> Challenge hash: \(challengeHash.prefix(8))... (length: \(challengeHash.count) bytes)"
            )

            do {
                print("  -> Calling DCAppAttestService.attestKey...")
                let result = try await service.attestKey(keyId, clientDataHash: challengeHash)
                print("  -> DCAppAttestService.attestKey succeeded.")
                return result
            } catch {
                print("❌ DCAppAttestService.attestKey failed: \(error)")
                if let dcError = error as? DCError {
                    print("❌ DCError code: \(dcError.code.rawValue)")
                    print("❌ DCError description: \(dcError.localizedDescription)")
                    print("❌ DCError domain: \((dcError as NSError).domain)")

                    // Handle specific error codes
                    switch dcError.code {
                    case DCError.Code.invalidInput:
                        print("❌ Invalid input provided to attestKey")
                    case DCError.Code.invalidKey:
                        print("❌ Invalid key - key may have been invalidated")
                        print("🔄 Clearing stored key and attestation flag for retry...")
                        _ = keychain.delete(for: self.keyId)
                        _ = keychain.delete(for: "hamrah_attestation_completed")
                        throw AttestationError.keyGenerationFailed(
                            "Key invalidated - cleared for regeneration")
                    case DCError.Code.serverUnavailable:
                        print("❌ Apple's attestation service is unavailable")
                    default:
                        print("❌ Unknown DCError code: \(dcError.code.rawValue)")
                    }
                }
                throw error
            }
        }

        private func generateAssertion(keyId: String, challenge: Data) async throws -> Data {
            // Create hash of challenge data
            let challengeHash = sha256(challenge)

            return try await service.generateAssertion(keyId, clientDataHash: challengeHash)
        }

        private func getAttestationChallenge(accessToken: String) async throws
            -> AttestationChallenge
        {
            print("  -> Step 3: Getting challenge from server...")
            let url = URL(string: "\(baseURL)/api/attestation/challenge")!
            var request = URLRequest(url: url)
            request.httpMethod = "POST"
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.setValue("Bearer \(accessToken)", forHTTPHeaderField: "Authorization")

            let body = [
                "platform": "ios",
                "bundle_id": Bundle.main.bundleIdentifier ?? "app.hamrah.ios",
            ]
            request.httpBody = try JSONEncoder().encode(body)

            let (data, response) = try await URLSession.shared.data(for: request)

            guard let httpResponse = response as? HTTPURLResponse,
                httpResponse.statusCode == 200
            else {
                throw AttestationError.challengeRequestFailed
            }

            let challengeResponse = try JSONDecoder().decode(
                AttestationChallengeResponse.self, from: data)

            guard challengeResponse.success,
                let challengeBase64 = challengeResponse.challenge,
                let challengeData = Data(base64Encoded: challengeBase64)
            else {
                throw AttestationError.invalidChallenge
            }

            print("  -> Step 3: Challenge received and decoded successfully.")
            return AttestationChallenge(
                challengeId: challengeResponse.challengeId,
                challengeData: challengeData
            )
        }

        private func submitAttestation(
            attestation: Data, keyId: String, challengeId: String, accessToken: String
        ) async throws {
            let url = URL(string: "\(baseURL)/api/attestation/verify")!
            var request = URLRequest(url: url)
            request.httpMethod = "POST"
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.setValue("Bearer \(accessToken)", forHTTPHeaderField: "Authorization")

            let body = [
                "attestation_object": attestation.base64EncodedString(),
                "key_id": keyId,
                "challenge_id": challengeId,
                "bundle_id": Bundle.main.bundleIdentifier ?? "app.hamrah.ios",
            ]
            request.httpBody = try JSONEncoder().encode(body)

            let (data, response) = try await URLSession.shared.data(for: request)

            guard let httpResponse = response as? HTTPURLResponse, httpResponse.statusCode == 200
            else {
                let statusCode = (response as? HTTPURLResponse)?.statusCode ?? -1
                print(
                    "❌ Attestation verification HTTP request failed with status code: \(statusCode)"
                )
                // Try to decode server error for more details
                if let verificationResponse = try? JSONDecoder().decode(
                    AttestationVerificationResponse.self, from: data)
                {
                    throw AttestationError.verificationFailed(
                        verificationResponse.error
                            ?? "Unknown server error with status code \(statusCode)")
                }
                throw AttestationError.verificationRequestFailed
            }

            let verificationResponse = try JSONDecoder().decode(
                AttestationVerificationResponse.self, from: data)

            guard verificationResponse.success else {
                throw AttestationError.verificationFailed(
                    verificationResponse.error ?? "Unknown server error")
            }
        }

        private func sha256(_ data: Data) -> Data {
            var hash = Data(count: Int(CC_SHA256_DIGEST_LENGTH))
            hash.withUnsafeMutableBytes { bytes in
                data.withUnsafeBytes { dataBytes in
                    _ = CC_SHA256(
                        dataBytes.baseAddress, CC_LONG(data.count),
                        bytes.bindMemory(to: UInt8.self).baseAddress)
                }
            }
            return hash
        }

        // MARK: - Public Reset Methods

        /// Resets attestation state - forces re-initialization on next login
        func resetAttestation() {
            _ = keychain.delete(for: keyId)
            _ = keychain.delete(for: "hamrah_attestation_completed")
            print("🔄 App Attestation reset completed")
        }

        /// Clears attestation completion flag - allows retry without generating new key
        func clearAttestationFlag() {
            _ = keychain.delete(for: "hamrah_attestation_completed")
            print("🔄 App Attestation flag cleared - will retry on next request")
        }

        /// Completely resets App Attestation state and forces re-initialization
        func forceReset() {
            print("🔄 Force resetting App Attestation...")
            _ = keychain.delete(for: keyId)
            _ = keychain.delete(for: "hamrah_attestation_completed")
            print("✅ App Attestation force reset completed - will regenerate key on next request")
        }

        /// Diagnoses current App Attestation state
        func diagnoseState() {
            print("🔍 App Attestation Diagnosis:")
            print("  -> Service supported: \(service.isSupported)")
            print("  -> Key stored: \(keychain.retrieveString(for: keyId) != nil)")
            if let storedKey = keychain.retrieveString(for: keyId) {
                print("  -> Stored key ID: \(storedKey.prefix(8))...")
            }
            print(
                "  -> Attestation completed flag: \(keychain.retrieveString(for: "hamrah_attestation_completed") ?? "false")"
            )
        }
    }

    // MARK: - Data Models

    struct AttestationChallenge {
        let challengeId: String
        let challengeData: Data
    }

    struct AttestationChallengeResponse: Codable {
        let success: Bool
        let challenge: String?
        let challengeId: String
        let error: String?

        enum CodingKeys: String, CodingKey {
            case success
            case challenge
            case challengeId = "challenge_id"
            case error
        }
    }

    struct AttestationVerificationResponse: Codable {
        let success: Bool
        let error: String?
    }

    // MARK: - Errors

    enum AttestationError: LocalizedError {
        case notSupported
        case keyStorageFailed
        case keyGenerationFailed(String)
        case challengeRequestFailed
        case invalidChallenge
        case verificationRequestFailed
        case verificationFailed(String)

        var errorDescription: String? {
            switch self {
            case .notSupported:
                return "App Attestation is not supported on this device"
            case .keyStorageFailed:
                return "Failed to store attestation key securely"
            case .keyGenerationFailed(let details):
                return "Failed to generate attestation key: \(details)"
            case .challengeRequestFailed:
                return "Failed to request attestation challenge"
            case .invalidChallenge:
                return "Invalid attestation challenge received"
            case .verificationRequestFailed:
                return "Failed to submit attestation for verification"
            case .verificationFailed(let error):
                return "Attestation verification failed: \(error)"
            }
        }
    }
#endif
