//
//  RootView.swift
//  hamrahIOS
//
//  Root view that handles progressive authentication flow
//

import SwiftUI

struct RootView: View {
    @StateObject private var nativeAuthManager = NativeAuthManager()
    @StateObject private var biometricManager = BiometricAuthManager()
    @State private var navPath = NavigationPath()

    var body: some View {
        NavigationStack(path: $navPath) {
            ProgressiveAuthView()
                .environmentObject(nativeAuthManager)
                .environmentObject(biometricManager)

        }
    }

    /// Helper to reset navigation to the root and show the inbox (when authenticated).
    func openInboxAsRoot() {
        navPath = NavigationPath()
    }
}

#Preview {
    RootView()
}
