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
        ProgressiveAuthView()
            .environmentObject(nativeAuthManager)
            .environmentObject(biometricManager)
            .onAppear {
                syncEngine.setAccessTokenRefresher { [weak nativeAuthManager] in
                    await nativeAuthManager?.accessTokenForServerRequest()
                }
            }
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
