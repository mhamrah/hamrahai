//
//  InboxViewModel.swift
//  hamrah-ios
//
//  ViewModel for managing inbox state and link operations
//

import Combine
import Foundation
import SwiftData

@MainActor
final class InboxViewModel: BaseViewModel {

    // MARK: - Published Properties

    @Published var searchText: String = ""
    @Published var selectedSort: LinkSort = .recent
    @Published var showFailedOnly: Bool = false
    @Published var selectedStatus: String?
    @Published var selectedTags: [String] = []

    @Published var syncing: Bool = false

    // MARK: - Dependencies

    private let syncEngine: SyncEngine
    private let api: LinkAPI
    private let modelContext: ModelContext

    // MARK: - Computed Properties

    var fetchDescriptor: FetchDescriptor<LinkEntity> {
        let searchTerm = searchText.trimmingCharacters(in: .whitespacesAndNewlines)
        let status = showFailedOnly ? "failed" : selectedStatus

        if searchTerm.isEmpty && status == nil && selectedTags.isEmpty {
            return LinkQueryDescriptors.all(limit: 50, sort: selectedSort)
        } else {
            return LinkQueryDescriptors.filtered(
                searchTerm: searchTerm.isEmpty ? nil : searchTerm,
                status: status,

                tags: selectedTags,
                sort: selectedSort,
                limit: 50
            )
        }
    }

    var hasActiveFilters: Bool {
        !searchText.isEmpty || showFailedOnly || selectedStatus != nil || !selectedTags.isEmpty

    }

    // MARK: - Initialization

    init(syncEngine: SyncEngine? = nil, api: LinkAPI? = nil, modelContext: ModelContext) {
        self.syncEngine = syncEngine ?? SyncEngine()
        self.api = api ?? SecureAPILinkClient()
        self.modelContext = modelContext
        super.init()
        setupSearchDebouncing()
    }

    // MARK: - Public Methods

    func refresh() async {
        setLoading(true)
        syncing = true

        do {
            syncEngine.triggerSync(reason: "pull-to-refresh")
            // Give some time for sync to start
            try await Task.sleep(nanoseconds: 500_000_000)  // 0.5 seconds
            syncing = false
            setLoading(false)
        } catch {
            syncing = false
            handleError(error)
        }
    }

    func openOriginal(_ link: LinkEntity) {
        PlatformBridge.openURL(link.canonicalUrl)
    }

    func deleteLink(_ link: LinkEntity) {
        let localId = link.localId
        let serverId = link.serverId

        Task { @MainActor in
            do {
                if let serverId {
                    try await api.deleteLink(serverId: serverId)
                }
                guard let link = self.link(withLocalId: localId) else { return }
                modelContext.delete(link)
                try modelContext.save()
            } catch {
                handleError(error)
            }
        }
    }

    func deleteLink(localId: UUID) {
        guard let link = link(withLocalId: localId) else { return }
        deleteLink(link)
    }

    func archiveLink(_ link: LinkEntity) {
        let localId = link.localId
        let serverId = link.serverId

        Task { @MainActor in
            do {
                if let serverId {
                    _ = try await api.updateLink(serverId: serverId, status: "archived")
                }
                guard let link = self.link(withLocalId: localId) else { return }
                link.status = "archived"
                link.updatedAt = Date()
                try modelContext.save()
            } catch {
                handleError(error)
            }
        }
    }

    func archiveLink(localId: UUID) {
        guard let link = link(withLocalId: localId) else { return }
        archiveLink(link)
    }

    func retrySync(for link: LinkEntity) {
        link.status = "queued"
        link.attempts = 0
        link.lastError = nil
        link.updatedAt = Date()

        do {
            try modelContext.save()
            syncEngine.triggerSync(reason: "retry")
        } catch {
            handleError(error)
        }
    }

    func retrySync(localId: UUID) {
        guard let link = link(withLocalId: localId) else { return }
        retrySync(for: link)
    }

    func clearAllFilters() {
        searchText = ""
        showFailedOnly = false
        selectedStatus = nil
        selectedTags.removeAll()

        selectedSort = .recent
    }

    func toggleTag(_ tagName: String) {
        if selectedTags.contains(tagName) {
            selectedTags.removeAll { $0 == tagName }
        } else {
            selectedTags.append(tagName)
        }
    }

    private func link(withLocalId localId: UUID) -> LinkEntity? {
        try? modelContext.fetch(
            FetchDescriptor<LinkEntity>(predicate: #Predicate { $0.localId == localId })
        ).first
    }

    // MARK: - Private Methods

    private func setupSearchDebouncing() {
        $searchText
            .debounce(for: .milliseconds(300), scheduler: RunLoop.main)
            .sink { _ in
                // Search text change will automatically trigger view updates
                // due to the computed fetchDescriptor property
            }
            .store(in: &cancellables)
    }
}

// MARK: - Preview Helpers

#if DEBUG
    extension InboxViewModel {
        static func preview() -> InboxViewModel {
            let container = AppModelSchema.makeInMemoryContainer()
            let context = ModelContext(container)

            // Add some sample data
            let sampleLink = LinkEntity(
                originalUrl: URL(string: "https://example.com")!,
                canonicalUrl: URL(string: "https://example.com")!,
                title: "Sample Link",
                snippet: "This is a sample link for preview purposes",
                status: "synced"
            )
            context.insert(sampleLink)

            return InboxViewModel(modelContext: context)
        }
    }
#endif
