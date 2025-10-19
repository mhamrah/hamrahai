/**
 AppAttestationManager+macOS.swift
 Hamrah (Multiplatform)

 macOS stub implementation of AppAttestationManager.

 Rationale:
 The iOS implementation (AppAttestationManager.swift) uses DeviceCheck / App Attest
 APIs (DCAppAttestService) and UIKit. Those frameworks are unavailable on macOS.
 To allow a single shared authentication / networking layer to compile for macOS,
 we provide a drop‑in replacement with the SAME public surface:

    - static let shared
    - func generateAttestationHeaders(for:)
    - func initializeAttestation(accessToken:)
    - func resetAttestation()

 The iOS logic adds cryptographic attestations. On macOS we instead return
 lightweight, clearly-marked, NON-SECURE headers so backend services can:
    * Detect platform (macOS)
    * Optionally downgrade trust / require alternative verification
    * Avoid rejecting requests outright

 IMPORTANT:
 - Backends MUST NOT treat these macOS headers as equivalent to iOS App Attestation.
 - If stronger Mac integrity is needed, integrate a notarized helper or future
   platform attestation APIs when available.

 Integration Notes:
 1. Wrap the original iOS-only file (AppAttestationManager.swift) in:
        #if os(iOS)
        ... existing implementation ...
        #endif
    so it does not compile on macOS.
 2. Leave this file unmodified; it safely compiles only on macOS due to the
    #if os(macOS) guard below.
 3. Calls to `initializeAttestation` on macOS are no-ops and will log once.

 The rest of the app (SecureAPIService, NativeAuthManager) can remain unchanged.
 */

#if os(macOS)

    import Foundation
    import CryptoKit
    import AppKit

    final class AppAttestationManager: ObservableObject {

        // MARK: - Singleton
        static let shared = AppAttestationManager()

        private init() {
            logOnce("macOS AppAttestationManager stub initialized (no secure attestation).")
        }

        // MARK: - Public API (Interface Parity)

        /// Generates NON-SECURE identification headers for macOS builds.
        /// These are ONLY soft identification signals and MUST NOT be trusted
        /// as proof of genuine app integrity.
        ///
        /// - Parameter challenge: Arbitrary request challenge data (ignored for security; only hashed for variability).
        /// - Returns: Dictionary of headers to attach to outbound requests.
        func generateAttestationHeaders(for challenge: Data) async throws -> [String: String] {
            let bundleId = Bundle.main.bundleIdentifier ?? "app.hamrah.macos"
            let appVersion =
                (Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String) ?? "unknown"
            let buildNumber =
                (Bundle.main.infoDictionary?["CFBundleVersion"] as? String) ?? "unknown"

            // Derive a lightweight, NON-CRYPTOGRAPHIC fingerprint (debug aid only).
            let hashSeed = Data((bundleId + appVersion + buildNumber).utf8) + challenge.prefix(32)
            let digest = sha256String(hashSeed)

            return [
                "X-Platform": "macOS",
                "X-App-Bundle-ID": bundleId,
                "X-App-Version": appVersion,
                "X-App-Build": buildNumber,
                "X-App-NonSecure-Fingerprint": digest,
                "X-App-Attestation-Mode": "none",  // explicit marker for backend logic
            ]
        }

        /// No-op on macOS (there is no App Attest). Called after login on iOS.
        func initializeAttestation(accessToken: String) async throws {
            // Intentionally left blank; logged only once to avoid spam.
            logOnce("initializeAttestation(accessToken:) called on macOS stub (no action).")
        }

        /// Resets any cached state. For parity only (does nothing here).
        func resetAttestation() {
            // No state to reset in the macOS stub.
        }

        // MARK: - Public Reset/Debug Parity Methods

        /// Clears attestation completion flag (no-op on macOS).
        func clearAttestationFlag() {
            // No persistent attestation flag on macOS stub.
            print("[AppAttestationManager macOS] clearAttestationFlag() no-op")
        }

        /// Completely resets App Attestation state (no-op on macOS).
        func forceReset() {
            // Nothing to reset in the macOS stub.
            print("[AppAttestationManager macOS] forceReset() no-op")
        }

        /// Diagnoses current (stub) App Attestation state on macOS.
        func diagnoseState() {
            print("[AppAttestationManager macOS] Diagnosis:")
            print("  -> Service supported: false (no App Attest on macOS)")
            print("  -> Key stored: false")
            print("  -> Attestation completed flag: false")
        }

        // MARK: - Internal Helpers

        private func sha256String(_ data: Data) -> String {
            let digest = SHA256.hash(data: data)
            return digest.map { String(format: "%02x", $0) }.joined()
        }

        private var didLog = Set<String>()
        private func logOnce(_ message: String) {
            if didLog.insert(message).inserted {
                print("[AppAttestationManager macOS] \(message)")
            }
        }
    }

#endif
