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

    var body: some View {
        ProgressiveAuthView()
            .environmentObject(nativeAuthManager)
            .environmentObject(biometricManager)
    }
}

#Preview {
    RootView()
        .environmentObject(
            SyncEngine(
                api: PreviewLinkAPI(),
                modelContainer: AppModelSchema.makeInMemoryContainer()
            )
        )
}
