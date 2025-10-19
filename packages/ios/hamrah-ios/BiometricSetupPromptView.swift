//
//  BiometricSetupPromptView.swift
//  hamrahIOS
//
//  Prompt view to encourage users to set up Face ID after login
//

import LocalAuthentication
import SwiftUI

struct BiometricSetupPromptView: View {
    @EnvironmentObject var biometricManager: BiometricAuthManager
    let onSetup: () -> Void
    let onSkip: () -> Void

    @State private var isSettingUp = false

    var body: some View {
        VStack(spacing: 32) {
            Spacer()

            // Icon and Title
            VStack(spacing: 20) {
                Image(systemName: biometricIconName)
                    .font(.system(size: 80))
                    .foregroundColor(.blue)

                Text("Set up \(biometricManager.biometricTypeString)")
                    .font(.largeTitle)
                    .fontWeight(.bold)
                    .multilineTextAlignment(.center)

                Text(
                    "Use \(biometricManager.biometricTypeString) for quick and secure access to your account"
                )
                .font(.body)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal)
            }

            Spacer()

            // Benefits
            VStack(spacing: 16) {
                FeatureBenefit(
                    icon: "bolt.fill",
                    title: "Fast & Convenient",
                    description: "Access your account instantly"
                )

                FeatureBenefit(
                    icon: "shield.fill",
                    title: "Secure",
                    description: "Your biometric data stays on your device"
                )

                FeatureBenefit(
                    icon: "lock.fill",
                    title: "Private",
                    description: "No passwords to remember or type"
                )
            }
            .padding(.horizontal)

            Spacer()

            // Action Buttons
            VStack(spacing: 12) {
                Button(action: setupBiometric) {
                    HStack {
                        if isSettingUp {
                            ProgressView()
                                .progressViewStyle(CircularProgressViewStyle(tint: .white))
                                .scaleEffect(0.8)
                        } else {
                            Image(systemName: biometricIconName)
                        }

                        Text(
                            isSettingUp
                                ? "Setting up..." : "Enable \(biometricManager.biometricTypeString)"
                        )
                    }
                    .font(.headline)
                    .foregroundColor(.white)
                    .frame(maxWidth: .infinity)
                    .frame(height: 56)
                    .background(Color.blue)
                    .cornerRadius(12)
                }
                .disabled(isSettingUp)

                Button("Maybe Later") {
                    onSkip()
                }
                .font(.subheadline)
                .foregroundColor(.secondary)
                .disabled(isSettingUp)
            }
            .padding(.horizontal, 32)

            Spacer()
        }
        .padding()
    }

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

    private func setupBiometric() {
        isSettingUp = true

        Task {
            let _ = await biometricManager.enableBiometricAuth()

            await MainActor.run {
                isSettingUp = false
                onSetup()
            }
        }
    }
}

struct FeatureBenefit: View {
    let icon: String
    let title: String
    let description: String

    var body: some View {
        HStack(spacing: 16) {
            Image(systemName: icon)
                .font(.title2)
                .foregroundColor(.blue)
                .frame(width: 30)

            VStack(alignment: .leading, spacing: 4) {
                Text(title)
                    .font(.headline)
                    .foregroundColor(.primary)

                Text(description)
                    .font(.subheadline)
                    .foregroundColor(.secondary)
            }

            Spacer()
        }
    }
}

#Preview {
    BiometricSetupPromptView(
        onSetup: { print("Setup") },
        onSkip: { print("Skip") }
    )
    .environmentObject(BiometricAuthManager())
}
