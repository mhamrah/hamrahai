//
//  BiometricAuthManager.swift
//  hamrahIOS
//
//  Cross-platform biometric authentication manager.
//  iOS / macOS (Touch ID / Face ID / Optic ID). Gracefully degrades when
//  LocalAuthentication framework or hardware is unavailable.
//
//  On macOS devices without Touch ID (or in CI), all biometric operations
//  report unavailable without throwing, allowing the app to still function.
//

import Combine
import Foundation
import LocalAuthentication
import SwiftUI

@MainActor
class BiometricAuthManager: ObservableObject {
    @Published var isBiometricEnabled = false
    @Published var biometricType: LABiometryType = .none
    @Published var isAuthenticating = false
    @Published var errorMessage: String?

    private let context = LAContext()
    private let biometricEnabledKey = "hamrah_biometric_enabled"

    init() {
        checkBiometricCapability()
        loadBiometricSettings()
    }

    // MARK: - Capability

    private func checkBiometricCapability() {
        var error: NSError?
        // Attempt evaluation (safe probe)
        guard context.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &error)
        else {
            if let e = error {
                print("⚠️ Biometrics not available: \(e.localizedDescription)")
            } else {
                print("⚠️ Biometrics not available (unknown reason)")
            }
            biometricType = .none
            return
        }
        biometricType = context.biometryType
        print("✅ Biometric type available: \(biometricTypeString)")
    }

    // MARK: - Computed Helpers

    var biometricTypeString: String {
        switch biometricType {
        case .faceID: return "Face ID"
        case .touchID: return "Touch ID"
        case .opticID: return "Optic ID"
        case .none: return "None"
        @unknown default: return "Unknown"
        }
    }

    var isAvailable: Bool {
        return biometricType != .none
    }

    // MARK: - Authentication

    func authenticateWithBiometrics(
        reason: String = "Authenticate to access your account"
    ) async -> Bool {
        guard isAvailable else {
            errorMessage = "Biometric authentication is not available on this device"
            return false
        }
        isAuthenticating = true
        errorMessage = nil

        do {
            let success = try await context.evaluatePolicy(
                .deviceOwnerAuthenticationWithBiometrics,
                localizedReason: reason
            )
            if success {
                print("✅ Biometric authentication successful")
                isAuthenticating = false
                return true
            }
        } catch let error as LAError {
            handleBiometricError(error)
        } catch {
            errorMessage = "Biometric authentication failed: \(error.localizedDescription)"
            print("❌ Biometric authentication error: \(error)")
        }

        isAuthenticating = false
        return false
    }

    private func handleBiometricError(_ error: LAError) {
        switch error.code {
        case .userCancel:
            errorMessage = "Authentication was cancelled"
        case .userFallback:
            errorMessage = "User chose fallback authentication"
        case .biometryNotAvailable:
            errorMessage = "Biometric authentication is not available"
        case .biometryNotEnrolled:
            errorMessage = "\(biometricTypeString) is not set up"
        case .biometryLockout:
            errorMessage = "Too many attempts. Use device passcode."
        case .authenticationFailed:
            errorMessage = "Failed to verify your identity"
        case .invalidContext:
            errorMessage = "Authentication context invalid"
        case .notInteractive:
            errorMessage = "Authentication not interactive"
        case .passcodeNotSet:
            errorMessage = "Device passcode not set"
        case .systemCancel:
            errorMessage = "Authentication cancelled by system"
        case .appCancel:
            errorMessage = "Authentication cancelled by app"
        case .invalidDimensions:
            errorMessage = "Invalid biometric data"
        #if os(watchOS) || os(macOS)
            case .watchNotAvailable:
                errorMessage = "Apple Watch unavailable"
        #endif
        #if os(iOS) && compiler(>=6.0)
            case .companionNotAvailable:
                if #available(iOS 18.0, *) {
                    errorMessage = "Companion device unavailable"
                } else {
                    errorMessage = "Unknown biometric error"
                }
        #endif
        case .biometryDisconnected:
            errorMessage = "Biometric sensor disconnected"
        case .touchIDNotAvailable:
            errorMessage = "Touch ID not available"
        case .touchIDNotEnrolled:
            errorMessage = "Touch ID not set up"
        case .touchIDLockout:
            errorMessage = "Touch ID locked. Use passcode."
        case .biometryNotPaired:
            errorMessage = "Biometric device not paired"
        @unknown default:
            errorMessage = "Unknown biometric error"
        }
        print("❌ Biometric error: \(errorMessage ?? "Unknown")")
    }

    // MARK: - Settings

    func enableBiometricAuth() async -> Bool {
        let success = await authenticateWithBiometrics(
            reason: "Enable \(biometricTypeString) for quick access"
        )
        if success {
            isBiometricEnabled = true
            saveBiometricSettings()
            print("✅ Biometric authentication enabled")
        }
        return success
    }

    func disableBiometricAuth() {
        isBiometricEnabled = false
        saveBiometricSettings()
        print("🔐 Biometric authentication disabled")
    }

    private func saveBiometricSettings() {
        UserDefaults.standard.set(isBiometricEnabled, forKey: biometricEnabledKey)
    }

    private func loadBiometricSettings() {
        isBiometricEnabled = UserDefaults.standard.bool(forKey: biometricEnabledKey)
    }

    // MARK: - Launch Flow

    func shouldRequireBiometricAuth() -> Bool {
        isBiometricEnabled && isAvailable
    }

    func authenticateForAppAccess() async -> Bool {
        guard shouldRequireBiometricAuth() else {
            print("✅ Biometric auth not required - skipping")
            return true
        }

        print("🔒 Performing biometric authentication for app access")
        let success = await authenticateWithBiometrics(
            reason: "Unlock Hamrah App with \(biometricTypeString)"
        )

        if success {
            print("✅ Biometric authentication successful for app access")
        } else {
            print("❌ Biometric authentication failed for app access")
        }

        return success
    }

    /// Reset error state - useful when retrying authentication
    func clearError() {
        errorMessage = nil
    }

    /// Check if biometric authentication is both enabled and available
    var isBiometricReadyForUse: Bool {
        return isBiometricEnabled && isAvailable
    }

    /// Force a fresh capability check (useful after settings changes)
    func recheckBiometricCapability() {
        checkBiometricCapability()
    }
}
