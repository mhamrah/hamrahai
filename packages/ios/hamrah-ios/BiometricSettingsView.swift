//
//  BiometricSettingsView.swift
//  hamrahIOS
//
//  Settings view for managing Face ID/Touch ID authentication
//

import SwiftUI
import LocalAuthentication

struct BiometricSettingsView: View {
    @EnvironmentObject var biometricManager: BiometricAuthManager
    @State private var showingAlert = false
    @State private var alertMessage = ""
    
    var body: some View {
        List {
            Section {
                HStack {
                    Image(systemName: biometricIconName)
                        .foregroundColor(.blue)
                        .font(.title2)
                    
                    VStack(alignment: .leading, spacing: 4) {
                        Text(biometricManager.biometricTypeString)
                            .font(.headline)
                        
                        if biometricManager.isAvailable {
                            Text("Use \(biometricManager.biometricTypeString) to quickly access your account")
                                .font(.caption)
                                .foregroundColor(.secondary)
                        } else {
                            Text("Not available on this device")
                                .font(.caption)
                                .foregroundColor(.red)
                        }
                    }
                    
                    Spacer()
                    
                    Toggle("", isOn: Binding(
                        get: { biometricManager.isBiometricEnabled },
                        set: { newValue in
                            if newValue {
                                enableBiometric()
                            } else {
                                biometricManager.disableBiometricAuth()
                            }
                        }
                    ))
                    .disabled(!biometricManager.isAvailable || biometricManager.isAuthenticating)
                }
                .opacity(biometricManager.isAvailable ? 1.0 : 0.6)
            } header: {
                Text("Biometric Authentication")
            } footer: {
                if biometricManager.isAvailable {
                    Text("When enabled, you can use \(biometricManager.biometricTypeString) to authenticate instead of entering your credentials each time.")
                } else {
                    Text("Biometric authentication requires \(biometricManager.biometricTypeString) to be set up in your device settings.")
                }
            }
            
            if biometricManager.isBiometricEnabled {
                Section {
                    Button(action: testBiometric) {
                        HStack {
                            Image(systemName: "checkmark.shield")
                                .foregroundColor(.green)
                            Text("Test \(biometricManager.biometricTypeString)")
                            Spacer()
                            if biometricManager.isAuthenticating {
                                ProgressView()
                                    .scaleEffect(0.8)
                            }
                        }
                    }
                    .disabled(biometricManager.isAuthenticating)
                } footer: {
                    Text("Test your biometric authentication to ensure it's working properly.")
                }
            }
            
            Section {
                VStack(alignment: .leading, spacing: 12) {
                    HStack {
                        Image(systemName: "info.circle")
                            .foregroundColor(.blue)
                        Text("About Biometric Security")
                            .font(.headline)
                    }
                    
                    Text("• Your biometric data never leaves your device")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    
                    Text("• We use your device's secure hardware to verify your identity")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    
                    Text("• You can always disable this feature in settings")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                .padding(.vertical, 8)
            }
        }
        .navigationTitle("Biometric Security")
        #if os(iOS)
        .navigationBarTitleDisplayMode(.inline)
        #endif
        .alert("Biometric Authentication", isPresented: $showingAlert) {
            Button("OK") { }
        } message: {
            Text(alertMessage)
        }
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
            return "lock.slash"
        @unknown default:
            return "questionmark"
        }
    }
    
    private func enableBiometric() {
        Task {
            let success = await biometricManager.enableBiometricAuth()
            
            if success {
                alertMessage = "\(biometricManager.biometricTypeString) has been enabled successfully!"
                showingAlert = true
            } else if let error = biometricManager.errorMessage {
                alertMessage = error
                showingAlert = true
            }
        }
    }
    
    private func testBiometric() {
        Task {
            let success = await biometricManager.authenticateWithBiometrics(
                reason: "Test your \(biometricManager.biometricTypeString) authentication"
            )
            
            if success {
                alertMessage = "\(biometricManager.biometricTypeString) test successful! ✅"
            } else {
                alertMessage = biometricManager.errorMessage ?? "Authentication test failed"
            }
            
            showingAlert = true
        }
    }
}

#Preview {
    NavigationView {
        BiometricSettingsView()
            .environmentObject(BiometricAuthManager())
    }
}