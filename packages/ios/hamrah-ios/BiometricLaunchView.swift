//
//  BiometricLaunchView.swift
//  hamrahIOS
//
//  View shown during biometric authentication on app launch
//

import SwiftUI

struct BiometricLaunchView: View {
    @EnvironmentObject private var biometricManager: BiometricAuthManager
    
    var body: some View {
        VStack(spacing: 32) {
            Spacer()
            
            // Biometric icon
            Image(systemName: biometricIconName)
                .font(.system(size: 80))
                .foregroundColor(.accentColor)
                .opacity(biometricManager.isAuthenticating ? 0.6 : 1.0)
                .animation(.easeInOut(duration: 1.0).repeatForever(autoreverses: true), value: biometricManager.isAuthenticating)
            
            VStack(spacing: 16) {
                Text("Unlock Hamrah")
                    .font(.largeTitle)
                    .fontWeight(.bold)
                
                Text("Use \(biometricManager.biometricTypeString) to access your account")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 32)
            }
            
            if let errorMessage = biometricManager.errorMessage {
                Text(errorMessage)
                    .font(.caption)
                    .foregroundColor(.red)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 32)
                    .padding(.top, 16)
            }
            
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color.appBackground)
    }
    
    private var biometricIconName: String {
        switch biometricManager.biometricTypeString {
        case "Face ID":
            return "faceid"
        case "Touch ID":
            return "touchid"
        case "Optic ID":
            return "opticid"
        default:
            return "lock.shield"
        }
    }
}

#Preview {
    BiometricLaunchView()
        .environmentObject(BiometricAuthManager())
}