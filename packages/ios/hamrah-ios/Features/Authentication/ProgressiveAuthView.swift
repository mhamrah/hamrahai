//
//  ProgressiveAuthView.swift
//  hamrahIOS
//
//  Explicit biometric authentication view that ensures biometric auth is required
//  every time the app becomes active when enabled. Handles token refresh and
//  ensures users rarely see the login screen.
//

import SwiftUI

struct ProgressiveAuthView: View {
    @EnvironmentObject private var authManager: NativeAuthManager
    @EnvironmentObject private var biometricManager: BiometricAuthManager
    @Environment(\.scenePhase) private var scenePhase

    @State private var biometricAuthPending = false
    @State private var showingBiometricPrompt = false
    @State private var hasCheckedBiometric = false
    @State private var hasPerformedInitialAuth = false
    @State private var isAuthenticating = false
    @State private var previousScenePhase: ScenePhase = .active

    var body: some View {
        Group {
            if biometricAuthPending {
                BiometricLaunchView()
                    .environmentObject(biometricManager)
                    .onAppear {
                        Task {
                            await handleBiometricAuthOnLaunch()
                        }
                    }
            } else if authManager.isAuthenticated {
                InboxView()
                    .environmentObject(authManager)
                    .environmentObject(biometricManager)
            } else {
                NativeLoginView()
                    .environmentObject(authManager)
                    .environmentObject(biometricManager)
            }
        }
        .onAppear {
            Task {
                await handleAppActivation()
            }
        }
        .onChange(of: scenePhase) { oldPhase, newPhase in
            handleScenePhaseChange(oldPhase: oldPhase, newPhase: newPhase)
        }
        .onChange(of: authManager.isAuthenticated) { oldValue, newValue in
            if newValue && !oldValue {
                // User just logged in - trigger authentication flow
                Task {
                    await handleLoginSuccess()
                }
            } else if !newValue && oldValue {
                // User logged out - reset state
                resetAuthenticationState()
            }
        }
    }

    // MARK: - Authentication Flow

    private func handleAppActivation() async {
        // Prevent multiple simultaneous authentication attempts
        guard !isAuthenticating else { return }
        isAuthenticating = true
        defer { isAuthenticating = false }

        await performAuthenticationFlow()
    }

    private func handleScenePhaseChange(oldPhase: ScenePhase, newPhase: ScenePhase) {
        previousScenePhase = oldPhase

        // Trigger biometric authentication when app becomes active from background/inactive
        // But not on the initial app launch (handled by onAppear)
        if (oldPhase == .background || oldPhase == .inactive) && newPhase == .active
            && hasPerformedInitialAuth
        {
            print("🔄 App became active - checking authentication requirements")
            Task {
                await handleAppActivation()
            }
        }
    }

    private func handleLoginSuccess() async {
        print("🎉 Login successful - performing authentication flow")
        await performAuthenticationFlow()
    }

    private func resetAuthenticationState() {
        print("🔄 Resetting authentication state")
        hasCheckedBiometric = false
        hasPerformedInitialAuth = false
        biometricAuthPending = false
    }

    private func performAuthenticationFlow() async {
        print("🔐 Starting authentication flow...")

        // Load stored authentication state
        await authManager.loadAuthenticationState()

        // Check if we have stored authentication
        guard authManager.isAuthenticated else {
            print("🚪 No stored authentication - showing login")
            await MainActor.run {
                hasPerformedInitialAuth = true
            }
            return
        }

        // Validate and refresh tokens if needed
        let tokenIsValid = await validateAndRefreshTokenIfNeeded()
        guard tokenIsValid else {
            print("🚫 Token validation failed - showing login")
            await MainActor.run {
                authManager.logout()
                hasPerformedInitialAuth = true
            }
            return
        }

        // Handle biometric authentication requirements
        if biometricManager.shouldRequireBiometricAuth() {
            print("🔒 Biometric authentication required")
            await handleBiometricAuthenticationFlow()
        } else {
            print("✅ Authentication complete - no biometric required")
            initializeAppAttestation()
        }

        await MainActor.run {
            hasPerformedInitialAuth = true
        }
    }

    private func initializeAppAttestation() {
        Task {
            if let token = authManager.accessToken {
                print("🔐 Initializing App Attestation if needed...")
                await SecureAPIService.shared.initializeAttestation(accessToken: token)
            }
        }
    }

    private func validateAndRefreshTokenIfNeeded() async -> Bool {
        // Check if token is expiring soon or needs validation
        if authManager.isTokenExpiringSoon() {
            print("🔄 Token is expiring soon - attempting refresh")
            let refreshSuccess = await authManager.refreshToken()
            if refreshSuccess {
                print("✅ Token refresh successful")
                return true
            } else {
                print("❌ Token refresh failed")
                return false
            }
        }

        // Validate current token
        let validationResult = await authManager.validateAccessToken()
        if validationResult {
            print("✅ Access token is valid")
            return true
        }

        print("🔄 Access token invalid - attempting refresh")
        let refreshSuccess = await authManager.refreshToken()
        if refreshSuccess {
            print("✅ Token refresh after validation failure successful")
            return true
        } else {
            print("❌ Token refresh after validation failure failed")
            return false
        }
    }

    private func handleBiometricAuthenticationFlow() async {
        await MainActor.run {
            biometricAuthPending = true
            hasCheckedBiometric = true
        }
    }

    private func checkBiometricAuthRequirement() {
        // Legacy method - now calls the new flow
        Task {
            await handleAppActivation()
        }
    }

    private func handleBiometricAuthOnLaunch() async {
        // Prevent multiple simultaneous auth attempts
        guard biometricAuthPending else { return }

        print("🔒 Performing biometric authentication")
        let success = await biometricManager.authenticateForAppAccess()

        await MainActor.run {
            biometricAuthPending = false

            if success {
                print("✅ Biometric authentication successful")
                initializeAppAttestation()
                hasCheckedBiometric = true
            } else {
                print("❌ Biometric authentication failed - logging out for security")
                // If biometric auth fails, log out the user for security
                authManager.logout()
                hasCheckedBiometric = false  // Allow re-checking after logout
            }
        }
    }
}

#Preview {
    ProgressiveAuthView()
        .environmentObject(NativeAuthManager())
        .environmentObject(BiometricAuthManager())
}
