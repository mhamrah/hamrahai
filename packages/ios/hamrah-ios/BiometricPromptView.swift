//
//  BiometricPromptView.swift
//  hamrahIOS
//
//  Biometric authentication prompt view for app launch
//

import LocalAuthentication
import SwiftUI

#if os(macOS)
    import AppKit
#endif

struct BiometricPromptView: View {
    @EnvironmentObject var biometricManager: BiometricAuthManager
    let onAuthenticated: () -> Void
    let onSkip: () -> Void

    @State private var showingError = false

    var body: some View {
        Group {
            VStack(spacing: platformVerticalSpacing) {
                Spacer(minLength: platformTopSpacer)

                // App Logo/Icon
                VStack(spacing: 20) {
                    Image(systemName: "person.crop.circle.fill")
                        .font(.system(size: 80))
                        .foregroundColor(.accentColor)
                        .accessibilityIdentifier("appLogo")

                    Text("Hamrah")
                        .font(.largeTitle)
                        .fontWeight(.bold)
                        .multilineTextAlignment(.center)
                        .accessibilityIdentifier("appTitle")
                }

                Spacer(minLength: 0)

                // Biometric Authentication Section
                VStack(spacing: 24) {
                    VStack(spacing: 16) {
                        Image(systemName: biometricIconName)
                            .font(.system(size: 60))
                            .foregroundColor(.blue)
                            .accessibilityIdentifier("biometricIcon")

                        Text("Unlock with \(biometricManager.biometricTypeString)")
                            .font(.title2)
                            .fontWeight(.semibold)
                            .multilineTextAlignment(.center)
                            .accessibilityIdentifier("unlockTitle")

                        Text(
                            "Use \(biometricManager.biometricTypeString) to securely access your account"
                        )
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal)
                        .accessibilityIdentifier("unlockSubtitle")
                    }

                    VStack(spacing: 16) {
                        // Primary biometric authentication button
                        Button(action: authenticateWithBiometric) {
                            HStack {
                                if biometricManager.isAuthenticating {
                                    ProgressView()
                                        .progressViewStyle(CircularProgressViewStyle(tint: .white))
                                        .scaleEffect(0.85)
                                } else {
                                    Image(systemName: biometricIconName)
                                }

                                Text(
                                    biometricManager.isAuthenticating
                                        ? "Authenticating..."
                                        : "Use \(biometricManager.biometricTypeString)")
                            }
                            .font(.headline)
                            .foregroundColor(.white)
                            .frame(maxWidth: .infinity)
                            .frame(height: 56)
                            .background(Color.accentColor.gradient)
                            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                            #if os(macOS)
                                .buttonStyle(.plain)
                            #endif
                        }
                        .disabled(biometricManager.isAuthenticating)
                        .accessibilityIdentifier("biometricPrimaryButton")

                        // Skip/Enter app button
                        Button(action: onSkip) {
                            Text("Enter app without \(biometricManager.biometricTypeString)")
                                .font(.subheadline)
                                .foregroundColor(.blue)
                                .underline()
                        }
                        .disabled(biometricManager.isAuthenticating)
                        .accessibilityIdentifier("biometricSkipButton")
                        #if os(macOS)
                            .buttonStyle(.plain)
                        #endif
                    }
                }
                .frame(maxWidth: 520)

                // Error message
                if let errorMessage = biometricManager.errorMessage {
                    Text(errorMessage)
                        .font(.caption)
                        .foregroundColor(.red)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal)
                        .transition(.opacity)
                        .accessibilityIdentifier("biometricErrorMessage")
                }

                Spacer(minLength: 0)

                // Footer
                VStack(spacing: 8) {
                    Text("Secure access with biometric authentication")
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .multilineTextAlignment(.center)

                    Text("Your biometric data stays securely on your device")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: 360)
                }
            }
            .padding(.horizontal, platformHorizontalPadding)
            .padding(.vertical, platformVerticalPadding)
            #if os(macOS)
                .frame(
                    minWidth: 520, idealWidth: 640, maxWidth: 820,
                    minHeight: 520, idealHeight: 640, maxHeight: 900,
                    alignment: .center)
            #endif
            .onAppear {
                // Automatically trigger biometric authentication when view appears
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
                    authenticateWithBiometric()
                }
            }
        }
    }

    // MARK: - Platform Tweaks

    private var platformHorizontalPadding: CGFloat {
        #if os(macOS)
            return 48
        #else
            return 32
        #endif
    }

    private var platformVerticalPadding: CGFloat {
        #if os(macOS)
            return 32
        #else
            return 20
        #endif
    }

    private var platformVerticalSpacing: CGFloat {
        #if os(macOS)
            return 48
        #else
            return 40
        #endif
    }

    private var platformTopSpacer: CGFloat {
        #if os(macOS)
            return 10
        #else
            return 0
        #endif
    }

    // MARK: - Icon

    private var biometricIconName: String {
        switch biometricManager.biometricType {
        case .faceID:
            return "faceid"
        case .touchID:
            return "touchid"
        case .opticID:
            return "opticid"
        case .none:
            return "lock"
        @unknown default:
            return "questionmark"
        }
    }

    // MARK: - Actions

    private func authenticateWithBiometric() {
        Task {
            let success = await biometricManager.authenticateForAppAccess()
            if success {
                onAuthenticated()
            }
            // If authentication fails, user can see the error and try again or skip
        }
    }
}

#Preview {
    BiometricPromptView(
        onAuthenticated: { print("Authenticated") },
        onSkip: { print("Skipped") }
    )
    .environmentObject(BiometricAuthManager())
}
