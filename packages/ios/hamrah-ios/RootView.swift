//
//  RootView.swift
//  hamrahIOS
//
//  Root view that handles progressive authentication flow
//

import SwiftUI

struct RootView: View {
    @EnvironmentObject private var syncEngine: SyncEngine
    @StateObject private var nativeAuthManager = NativeAuthManager()
    @StateObject private var biometricManager = BiometricAuthManager()

    var body: some View {
        #if DEBUG
            if ProcessInfo.processInfo.arguments.contains("--ui-testing-authenticated") {
                AuthenticatedMainView()
                    .environmentObject(nativeAuthManager)
                    .environmentObject(biometricManager)
            } else {
                authenticatedRoot
            }
        #else
            authenticatedRoot
        #endif
    }

    private var authenticatedRoot: some View {
        ProgressiveAuthView()
            .environmentObject(nativeAuthManager)
            .environmentObject(biometricManager)
    }
}

#if DEBUG
#Preview {
    RootView()
        .environmentObject(
            SyncEngine(
                api: PreviewLinkAPI(),
                modelContainer: AppModelSchema.makeInMemoryContainer()
            )
        )
}
#endif
