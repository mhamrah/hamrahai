//
//  OptimizedInboxView.swift
//  hamrah-ios
//
//  Enhanced inbox view with improved performance and UX
//

import SwiftData
import SwiftUI

struct OptimizedInboxView: View {
    @Environment(\.modelContext) private var modelContext

    var body: some View {
        InnerOptimizedInboxView(modelContext: modelContext)
    }
}

private struct InnerOptimizedInboxView: View {
    let modelContext: ModelContext
    @StateObject private var viewModel: InboxViewModel
    @Query private var links: [LinkEntity]

    @State private var showingFilterSheet = false
    @State private var selectedLink: LinkEntity?

    init(modelContext: ModelContext) {
        self.modelContext = modelContext
        self._viewModel = StateObject(wrappedValue: InboxViewModel(modelContext: modelContext))
        self._links = Query(LinkQueryDescriptors.recent())
    }

    var body: some View {
        NavigationStack {
            ZStack {
                if viewModel.isLoading && links.isEmpty {
                    loadingView
                } else if links.isEmpty {
                    emptyStateView
                } else {
                    linksList
                }

                if viewModel.syncing {
                    syncingIndicator
                }
            }
            .navigationTitle("Inbox")
            .toolbar {
                toolbarContent
            }
            .searchable(text: $viewModel.searchText, prompt: "Search links...")
            .refreshable {
                await viewModel.refresh()
            }
            .sheet(isPresented: $showingFilterSheet) {
                FilterSheet(viewModel: viewModel)
            }
            .platformAlert(
                isPresented: .constant(viewModel.errorMessage != nil),
                title: "Error",
                message: viewModel.errorMessage,
                primaryButton: .default("OK") {
                    viewModel.clearError()
                }
            )
        }
        .onAppear {
            setupNotificationObservers()
        }
    }

    // MARK: - View Components

    @ViewBuilder
    private var linksList: some View {
        List {
            if viewModel.hasActiveFilters {
                activeFiltersSection
            }

            ForEach(links, id: \.localId) { link in
                LinkCard(
                    link: link,
                    onTap: {
                        selectedLink = link
                    },
                    onOpenOriginal: {
                        viewModel.openOriginal(link)
                    }
                )
                .listRowSeparator(.hidden)
                .listRowInsets(
                    EdgeInsets(
                        top: Theme.Spacing.small,
                        leading: Theme.Spacing.medium,
                        bottom: Theme.Spacing.small,
                        trailing: Theme.Spacing.medium
                    ))
            }

            if links.count >= 50 {
                loadMoreSection
            }
        }
        .listStyle(.plain)
        .navigationDestination(item: $selectedLink) { link in
            LinkDetailView(link: link)
        }
    }

    @ViewBuilder
    private var activeFiltersSection: some View {
        VStack(alignment: .leading, spacing: Theme.Spacing.small) {
            HStack {
                Text("Active Filters")
                    .font(Theme.Typography.caption)
                    .foregroundColor(Theme.Colors.secondaryText)

                Spacer()

                Button("Clear All") {
                    viewModel.clearAllFilters()
                }
                .font(Theme.Typography.caption)
                .foregroundColor(Theme.Colors.primary)
            }

            LazyVGrid(
                columns: [
                    GridItem(.flexible()),
                    GridItem(.flexible()),
                ], spacing: Theme.Spacing.xsmall
            ) {
                if viewModel.showFailedOnly {
                    FilterChip(title: "Failed Only", isActive: true) {
                        viewModel.showFailedOnly = false
                    }
                }

                ForEach(viewModel.selectedTags, id: \.self) { tag in
                    FilterChip(title: tag, isActive: true) {
                        viewModel.toggleTag(tag)
                    }
                }
            }
        }
        .padding(.horizontal, Theme.Spacing.medium)
        .padding(.vertical, Theme.Spacing.small)
        .background(Theme.Colors.secondaryBackground)
        .listRowInsets(EdgeInsets())
    }

    @ViewBuilder
    private var loadMoreSection: some View {
        HStack {
            Spacer()
            Text("Load more links...")
                .font(Theme.Typography.caption)
                .foregroundColor(Theme.Colors.secondaryText)
            Spacer()
        }
        .padding()
    }

    @ViewBuilder
    private var emptyStateView: some View {
        ContentUnavailableView {
            Label("No Links Yet", systemImage: Theme.Icons.link)
        } description: {
            Text("Share links from any app to add them here.")
        } actions: {
            PlatformButton("Learn More", systemImage: "questionmark.circle") {
                // Open help or onboarding
            }
        }
    }

    @ViewBuilder
    private var loadingView: some View {
        VStack(spacing: Theme.Spacing.medium) {
            ProgressView()
                .scaleEffect(1.2)
            Text("Loading links...")
                .font(Theme.Typography.subheadline)
                .foregroundColor(Theme.Colors.secondaryText)
        }
    }

    @ViewBuilder
    private var syncingIndicator: some View {
        VStack {
            Spacer()
            HStack {
                ProgressView()
                    .scaleEffect(0.8)
                Text("Syncing...")
                    .font(Theme.Typography.caption)
            }
            .padding(.horizontal, Theme.Spacing.medium)
            .padding(.vertical, Theme.Spacing.small)
            .background(Theme.Colors.cardBackground)
            .cornerRadius(Theme.CornerRadius.medium)
            .shadow(
                color: Theme.Shadow.card.color,
                radius: Theme.Shadow.card.radius,
                x: Theme.Shadow.card.x,
                y: Theme.Shadow.card.y
            )
        }
        .padding(.bottom, Theme.Spacing.large)
    }

    @ToolbarContentBuilder
    private var toolbarContent: some ToolbarContent {
        #if os(iOS)
            ToolbarItemGroup(placement: .navigationBarLeading) {
                Menu {
                    sortPicker
                    Divider()
                    filterOptions
                } label: {
                    Label("Sort & Filter", systemImage: Theme.Icons.filter)
                }
            }

            ToolbarItemGroup(placement: .navigationBarTrailing) {
                Button {
                    showingFilterSheet = true
                } label: {
                    Image(systemName: "slider.horizontal.3")
                }
            }
        #elseif os(macOS)
            ToolbarItemGroup(placement: .primaryAction) {
                Menu {
                    sortPicker
                    Divider()
                    filterOptions
                } label: {
                    Label("Sort & Filter", systemImage: Theme.Icons.filter)
                }
            }

            ToolbarItemGroup(placement: .secondaryAction) {
                Button {
                    showingFilterSheet = true
                } label: {
                    Image(systemName: "slider.horizontal.3")
                }
            }
        #endif
    }

    @ViewBuilder
    private var sortPicker: some View {
        Picker("Sort", selection: $viewModel.selectedSort) {
            ForEach(LinkSort.allCases, id: \.self) { sort in
                Text(sort.title).tag(sort)
            }
        }
    }

    @ViewBuilder
    private var filterOptions: some View {
        Toggle(isOn: $viewModel.showFailedOnly) {
            Label("Show Failed Only", systemImage: Theme.Icons.warning)
        }
    }

    // MARK: - Helper Methods

    private func setupNotificationObservers() {
        NotificationCenter.default.addObserver(
            forName: .retryLinkSync,
            object: nil,
            queue: .main
        ) { notification in
            if let link = notification.object as? LinkEntity {
                Task { @MainActor in
                    viewModel.retrySync(for: link)
                }
            }
        }

        NotificationCenter.default.addObserver(
            forName: .deleteLinkRequest,
            object: nil,
            queue: .main
        ) { notification in
            if let link = notification.object as? LinkEntity {
                Task { @MainActor in
                    viewModel.deleteLink(link)
                }
            }
        }
    }
}

// MARK: - Supporting Views

struct FilterChip: View {
    let title: String
    let isActive: Bool
    let onTap: () -> Void

    var body: some View {
        Button(action: onTap) {
            HStack(spacing: Theme.Spacing.xsmall) {
                Text(title)
                if isActive {
                    Image(systemName: "xmark")
                        .font(.caption2)
                }
            }
            .font(Theme.Typography.caption)
            .padding(.horizontal, Theme.Spacing.small)
            .padding(.vertical, 4)
            .background(isActive ? Theme.Colors.primary : Theme.Colors.secondaryBackground)
            .foregroundColor(isActive ? .white : Theme.Colors.primaryText)
            .cornerRadius(Theme.CornerRadius.small)
        }
        .buttonStyle(PlainButtonStyle())
    }
}

struct FilterSheet: View {
    @ObservedObject var viewModel: InboxViewModel
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Form {
                Section("Sort") {
                    Picker("Sort Order", selection: $viewModel.selectedSort) {
                        ForEach(LinkSort.allCases, id: \.self) { sort in
                            Text(sort.title).tag(sort)
                        }
                    }
                    .pickerStyle(.segmented)
                }

                Section("Filters") {
                    Toggle("Failed Links Only", isOn: $viewModel.showFailedOnly)

                    // Add more filter options here
                }

                Section {
                    PlatformButton("Clear All Filters", style: .secondary) {
                        viewModel.clearAllFilters()
                    }
                }
            }
            .navigationTitle("Filters")
            #if os(iOS)
                .navigationBarTitleDisplayMode(.inline)
            #endif
            .toolbar {
                #if os(iOS)
                    ToolbarItem(placement: .navigationBarTrailing) {
                        Button("Done") {
                            dismiss()
                        }
                    }
                #elseif os(macOS)
                    ToolbarItem(placement: .primaryAction) {
                        Button("Done") {
                            dismiss()
                        }
                    }
                #endif
            }
        }
    }
}

// MARK: - Preview

#Preview {
    OptimizedInboxView()
}
