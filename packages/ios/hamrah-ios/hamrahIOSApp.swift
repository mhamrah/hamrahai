//
//  hamrahIOSApp.swift
//  hamrahIOS
//
//  Created by Mike Hamrah on 8/10/25.
//

import SwiftData
import SwiftUI

// Using AppModelSchema for unified schema across targets

#if os(iOS)
    import BackgroundTasks
#endif
#if canImport(GoogleSignIn)
    import GoogleSignIn
#endif

@main
struct hamrahIOSApp: App {
    @Environment(\.scenePhase) private var scenePhase
    private let sharedModelContainer: ModelContainer
    @StateObject private var syncEngine: SyncEngine

    // Background sync registration - iOS only
    init() {
        let modelContainer = Self.makeModelContainer()
        self.sharedModelContainer = modelContainer
        let syncEngine = SyncEngine(modelContainer: modelContainer)
        self._syncEngine = StateObject(wrappedValue: syncEngine)

        #if os(iOS)
            #if !targetEnvironment(simulator)
                Self.registerBackgroundSync(using: syncEngine)
                print("🗓️ Scheduling background sync task on device...")
                Self.scheduleBackgroundSync()
            #else
                print("ℹ️ Skipping BGTask registration on Simulator.")
            #endif
        #endif
    }

    private static func makeModelContainer() -> ModelContainer {
        #if DEBUG
            AppModelSchema.makeSharedContainerWithRecovery()
        #else
            (try? AppModelSchema.makeSharedContainer()) ?? AppModelSchema.makeInMemoryContainer()
        #endif
    }

    #if os(iOS)
        private static func registerBackgroundSync(using syncEngine: SyncEngine) {
            BGTaskScheduler.shared.register(
                forTaskWithIdentifier: "app.hamrah.ios.sync", using: nil
            ) { task in
                Task {
                    await syncEngine.runSyncNow(reason: "background")
                    task.setTaskCompleted(success: true)
                    Self.scheduleBackgroundSync()
                }
            }
        }

        private static func scheduleBackgroundSync() {
            #if targetEnvironment(simulator)
                print("ℹ️ Skipping BGProcessingTask scheduling on Simulator.")
            #else
                print("📝 Preparing BGProcessingTask request for 'app.hamrah.ios.sync'")
                let request = BGProcessingTaskRequest(identifier: "app.hamrah.ios.sync")
                request.requiresNetworkConnectivity = true
                request.requiresExternalPower = false
                do {
                    try BGTaskScheduler.shared.submit(request)
                } catch {
                    print("Failed to submit BGProcessingTask: \(error)")
                }
            #endif
        }
    #endif

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(syncEngine)
                .task {
                    print(
                        "🌐 API baseURL: \(APIConfiguration.shared.baseURL) [env=\(APIConfiguration.shared.currentEnvironment.rawValue)]"
                    )
                    syncEngine.triggerSync(reason: "app_launch")
                }
                .onOpenURL { url in
                    // Handle deep link URLs (OAuth callback)
                    print("Received URL: \(url)")
                    // Google Sign-In URL handling not required here for modern SDK; proceed to deep link router.
                    let routed = DeepLinkRouter.handle(url) { reason in
                        syncEngine.triggerSync(reason: reason)
                    }
                    print("🔗 DeepLinkRouter handled: \(routed)")
                    if !routed {
                        syncEngine.triggerSync(reason: "open_url")
                    }
                }
        }
        .modelContainer(sharedModelContainer)
        .onChange(of: scenePhase) { _, newPhase in
            if newPhase == .active {
                syncEngine.triggerSync(reason: "app_active")
            }
        }
    }
}
