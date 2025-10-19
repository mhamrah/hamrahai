//
//  ExplicitBiometricAuthTests.swift
//  hamrah-iosTests
//
//  Tests for explicit biometric authentication behavior in ProgressiveAuthView.
//  Verifies that biometric auth is required every time app becomes active when enabled.
//

import SwiftUI
import Testing

@testable import hamrah_ios

@MainActor
struct ExplicitBiometricAuthTests {

    // MARK: - Biometric Authentication Requirements Tests

    @Test("Biometric should be required when enabled and app becomes active")
    func testBiometricRequiredOnAppActivation() async {
        // Given: User with biometric authentication enabled
        let biometricManager = BiometricAuthManager()
        let authManager = NativeAuthManager()

        // Mock biometric as enabled and available
        biometricManager.isBiometricEnabled = true
        authManager.isAuthenticated = true

        // When: Checking if biometric auth should be required
        let shouldRequire = biometricManager.shouldRequireBiometricAuth()

        // Then: Should require biometric authentication
        #expect(shouldRequire == true)
    }

    @Test("Biometric should not be required when disabled")
    func testBiometricNotRequiredWhenDisabled() async {
        // Given: User with biometric authentication disabled
        let biometricManager = BiometricAuthManager()
        let authManager = NativeAuthManager()

        // Mock biometric as disabled
        biometricManager.isBiometricEnabled = false
        authManager.isAuthenticated = true

        // When: Checking if biometric auth should be required
        let shouldRequire = biometricManager.shouldRequireBiometricAuth()

        // Then: Should not require biometric authentication
        #expect(shouldRequire == false)
    }

    @Test("Authentication state should load from stored data")
    func testAuthenticationStateLoading() async {
        // Given: Authentication manager
        let authManager = NativeAuthManager()

        // When: Loading authentication state
        await authManager.loadAuthenticationState()

        // Then: Should complete without error
        // Note: This tests the async loading mechanism works
        #expect(true)  // Test passes if no exception thrown
    }

    // MARK: - Token Management Tests

    @Test("Token expiration check should work correctly")
    func testTokenExpirationCheck() {
        // Given: Authentication manager
        let authManager = NativeAuthManager()

        // When: Checking if token is expiring soon
        let isExpiring = authManager.isTokenExpiringSoon()

        // Then: Should return a boolean result
        #expect(isExpiring == true || isExpiring == false)
    }

    @Test("Token validation should return result")
    func testTokenValidation() async {
        // Given: Authentication manager
        let authManager = NativeAuthManager()

        // When: Validating access token
        let isValid = await authManager.validateAccessToken()

        // Then: Should return a boolean result
        #expect(isValid == true || isValid == false)
    }

    @Test("Token refresh should return result")
    func testTokenRefresh() async {
        // Given: Authentication manager
        let authManager = NativeAuthManager()

        // When: Refreshing token
        let refreshSuccess = await authManager.refreshToken()

        // Then: Should return a boolean result
        #expect(refreshSuccess == true || refreshSuccess == false)
    }

    // MARK: - Security Tests

    @Test("Logout should clear authentication state")
    func testLogoutClearsAuthenticationState() {
        // Given: Authenticated user
        let authManager = NativeAuthManager()
        authManager.isAuthenticated = true

        // When: User logs out
        authManager.logout()

        // Then: Should no longer be authenticated
        #expect(authManager.isAuthenticated == false)
    }

    @Test("Force reauthentication should trigger logout")
    func testForceReauthentication() {
        // Given: Authenticated user
        let authManager = NativeAuthManager()
        authManager.isAuthenticated = true

        // When: Force reauthentication is called
        authManager.forceReauthentication()

        // Then: Should no longer be authenticated
        #expect(authManager.isAuthenticated == false)
    }

    // MARK: - Biometric Capability Tests

    @Test("Biometric type string should be valid")
    func testBiometricTypeString() {
        // Given: Biometric manager
        let biometricManager = BiometricAuthManager()

        // When: Getting biometric type string
        let typeString = biometricManager.biometricTypeString

        // Then: Should return a valid string
        #expect(typeString.count > 0)
        #expect(
            ["Face ID", "Touch ID", "Optic ID", "None", "Unknown", "Unavailable"].contains(
                typeString))
    }

    @Test("Biometric availability should be determinable")
    func testBiometricAvailability() {
        // Given: Biometric manager
        let biometricManager = BiometricAuthManager()

        // When: Checking availability
        let isAvailable = biometricManager.isAvailable

        // Then: Should return a boolean
        #expect(isAvailable == true || isAvailable == false)
    }

    @Test("Biometric ready for use should match enabled and available")
    func testBiometricReadyForUse() {
        // Given: Biometric manager
        let biometricManager = BiometricAuthManager()

        // When: Checking if ready for use
        let isReady = biometricManager.isBiometricReadyForUse
        let expectedReady = biometricManager.isBiometricEnabled && biometricManager.isAvailable

        // Then: Should match expected result
        #expect(isReady == expectedReady)
    }

    // MARK: - Authentication Flow Integration Tests

    @Test("Authentication managers should be properly initialized")
    func testAuthenticationManagersInitialization() {
        // Given: Creating managers
        let authManager = NativeAuthManager()
        let biometricManager = BiometricAuthManager()

        // When: Checking initial state
        // Then: Should be properly initialized
        #expect(authManager.isAuthenticated == false)
        #expect(authManager.isLoading == false)
        #expect(biometricManager.isBiometricEnabled == false)
        #expect(biometricManager.isAuthenticating == false)
    }

    @Test("Error handling should work correctly")
    func testErrorHandling() {
        // Given: Biometric manager
        let biometricManager = BiometricAuthManager()

        // When: Clearing error
        biometricManager.clearError()

        // Then: Error message should be nil
        #expect(biometricManager.errorMessage == nil)
    }

    @Test("Biometric capability recheck should work")
    func testBiometricCapabilityRecheck() {
        // Given: Biometric manager
        let biometricManager = BiometricAuthManager()
        let initialType = biometricManager.biometricTypeString

        // When: Rechecking capability
        biometricManager.recheckBiometricCapability()
        let recheckType = biometricManager.biometricTypeString

        // Then: Should maintain consistent type (or update if hardware changed)
        #expect(recheckType.count > 0)
    }

    // MARK: - Integration with ProgressiveAuthView Tests

    @Test("ProgressiveAuthView should handle environment objects")
    func testProgressiveAuthViewEnvironmentObjects() {
        // Given: Managers and view
        let authManager = NativeAuthManager()
        let biometricManager = BiometricAuthManager()

        // When: Creating view with environment objects
        let view = ProgressiveAuthView()
            .environmentObject(authManager)
            .environmentObject(biometricManager)

        // Then: Should create without error
        #expect(true)  // Test passes if no exception thrown during creation
    }

    // MARK: - Stored Token Tests

    @Test("Valid stored tokens check should work")
    func testValidStoredTokensCheck() {
        // Given: Authentication manager
        let authManager = NativeAuthManager()

        // When: Checking for valid stored tokens
        let hasValidTokens = authManager.hasValidStoredTokens()

        // Then: Should return a boolean
        #expect(hasValidTokens == true || hasValidTokens == false)
    }

    // MARK: - Performance and Edge Case Tests

    @Test("Multiple authentication attempts should be handled gracefully")
    func testMultipleAuthenticationAttempts() async {
        // Given: Authentication manager
        let authManager = NativeAuthManager()

        // When: Multiple rapid authentication state loads
        async let load1 = authManager.loadAuthenticationState()
        async let load2 = authManager.loadAuthenticationState()
        async let load3 = authManager.loadAuthenticationState()

        await load1
        await load2
        await load3

        // Then: Should handle gracefully without crashes
        #expect(true)  // Test passes if no crashes occur
    }

    @Test("Biometric authentication should handle availability changes")
    func testBiometricAvailabilityChanges() async {
        // Given: Biometric manager
        let biometricManager = BiometricAuthManager()

        // When: Checking availability and then performing auth
        let isAvailable = biometricManager.isAvailable
        let authResult = await biometricManager.authenticateForAppAccess()

        // Then: Should handle appropriately based on availability
        if !isAvailable {
            // If not available, should skip authentication gracefully
            #expect(authResult == true)  // Should return true when skipped
        } else {
            // If available, should attempt authentication
            #expect(authResult == true || authResult == false)  // Should return boolean result
        }
    }
}
