import SwiftData
import SwiftUI
import WebKit

#if os(iOS)
    import QuickLook
#endif

// MARK: - Inbox (List of Links)

struct InboxView: View {
    @Environment(\.modelContext) private var modelContext
    @Environment(\.openURL) private var openURL
    @EnvironmentObject var authManager: NativeAuthManager
    @EnvironmentObject var biometricManager: BiometricAuthManager
    @State private var searchText: String = ""
    @State private var sort: LinkSort = .recent
    @State private var showFailedOnly: Bool = false
    @State private var syncing: Bool = false

    // Query all links
    @Query var allLinks: [LinkEntity]

    // Computed property for dynamic filtering and sorting
    var links: [LinkEntity] {
        var filtered = allLinks
        if !searchText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            let term = searchText
            filtered = filtered.filter {
                ($0.title ?? "").contains(term)
                    || $0.originalUrl.absoluteString.contains(term)
                    || ($0.snippet ?? "").contains(term)
            }
        } else if showFailedOnly {
            filtered = filtered.filter { $0.status == "failed" }
        }
        switch sort {
        case .recent:
            filtered.sort { $0.updatedAt > $1.updatedAt }
        case .title:
            filtered.sort { ($0.title ?? "") < ($1.title ?? "") }
        case .domain:
            filtered.sort { ($0.canonicalUrl.host ?? "") < ($1.canonicalUrl.host ?? "") }
        case .created:
            filtered.sort { $0.createdAt > $1.createdAt }
        @unknown default:
            break
        }
        return filtered
    }

    init() {}

    var body: some View {
        NavigationStack {
            List {
                if links.isEmpty {
                    ContentUnavailableView(
                        "No links yet",
                        systemImage: "tray",
                        description: Text("Share links from any app to add them here."))
                } else {
                    ForEach(links, id: \.localId) { link in
                        NavigationLink(value: link.localId) {
                            LinkRowView(link: link)
                        }
                        .contextMenu {
                            Button {
                                openOriginal(link)
                            } label: {
                                Label("Open Original", systemImage: "safari")
                            }

                        }
                    }
                }
            }
            .refreshable { await runSync() }
            .navigationTitle("Inbox")
            .modifier(
                InboxToolbarModifier(
                    sort: $sort, showFailedOnly: $showFailedOnly, syncing: syncing, runSync: runSync
                )
            )
            #if os(iOS)
                .searchable(
                    text: $searchText, placement: .navigationBarDrawer, prompt: "Search links")
            #else
                .searchable(text: $searchText, prompt: "Search links")
            #endif
            .navigationDestination(for: UUID.self) { id in
                if let link = links.first(where: { $0.localId == id }) {
                    LinkDetailView(link: link)
                } else {
                    Text("Link not found")
                }
            }
        }
    }

    private func openOriginal(_ link: LinkEntity) {
        openURL(link.canonicalUrl)
    }

    private func runSync() async {
        syncing = true
        defer { syncing = false }
        await SyncEngine().runSyncNow(reason: "inbox_pull_to_refresh")
        // Re-evaluate query after sync
        // Instead of assigning to _links, which is immutable, trigger a state change
        // by updating a dummy state variable or by toggling a boolean
        // For now, just set syncing to false (already handled by defer)
    }
}

// MARK: - Row

struct LinkRowView: View {
    let link: LinkEntity

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            StatusDot(status: link.status)
                .padding(.top, 5)
            VStack(alignment: .leading, spacing: 4) {
                Text(link.title?.isEmpty == false ? link.title! : link.canonicalUrl.absoluteString)
                    .font(.headline)
                    .lineLimit(2)
                HStack(spacing: 8) {
                    if let host = link.canonicalUrl.host {
                        Text(host).foregroundStyle(.secondary)
                    }
                    if let last = relativeDate(link.updatedAt) {
                        Text("•").foregroundStyle(.secondary)
                        Text(last).foregroundStyle(.secondary)
                    }
                }
                .font(.caption)
                if let snippet = link.snippet, !snippet.isEmpty {
                    Text(snippet)
                        .lineLimit(2)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
            }
            Spacer(minLength: 0)

        }
        .contentShape(Rectangle())
    }

    private func relativeDate(_ date: Date) -> String? {
        let fmt = RelativeDateTimeFormatter()
        fmt.unitsStyle = .abbreviated
        return fmt.localizedString(for: date, relativeTo: Date())
    }
}

struct StatusDot: View {
    let status: String
    var color: Color {
        switch status {
        case "queued": return .yellow
        case "syncing": return .blue
        case "synced": return .green
        case "failed": return .red
        default: return .gray
        }
    }
    var body: some View {
        Circle()
            .fill(color)
            .frame(width: 10, height: 10)
            .accessibilityHidden(true)
    }
}

// MARK: - Detail

struct LinkDetailView: View {
    @Environment(\.modelContext) private var modelContext
    @Environment(\.openURL) private var openURL

    @State private var showShareSheet = false

    let link: LinkEntity

    // Normalize optional/non-optional tags to a concrete array
    private var tagsArray: [TagEntity] { link.tags }

    var body: some View {
        VStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 8) {
                Text(link.title?.isEmpty == false ? link.title! : link.canonicalUrl.absoluteString)
                    .font(.title3)
                    .fontWeight(.semibold)
                    .multilineTextAlignment(.leading)
                    .lineLimit(3)
                HStack(spacing: 8) {
                    if let host = link.canonicalUrl.host {
                        Text(host).foregroundStyle(.secondary).font(.subheadline)
                    }
                    Spacer()
                    StatusDot(status: link.status)
                        .accessibilityLabel("Status")
                }
                if let snippet = link.summaryShort ?? link.snippet, !snippet.isEmpty {
                    Text(snippet)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .lineLimit(3)
                }
                if !tagsArray.isEmpty {
                    TagCloud(tags: tagsArray.map { $0.name })
                }
            }
            .padding()
            Divider()
            WebView(url: link.canonicalUrl)
                .ignoresSafeArea(edges: .bottom)
        }
        .navigationTitle(link.title ?? domainOrURLString(link))
        #if os(iOS)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItemGroup(placement: .navigationBarTrailing) {
                    Button {
                        openOriginal()
                    } label: {
                        Label("Open Original", systemImage: "safari")
                    }

                }
            }
        #endif

    }

    private func openOriginal() {
        openURL(link.canonicalUrl)
    }

    private func domainOrURLString(_ link: LinkEntity) -> String {
        link.canonicalUrl.host ?? link.canonicalUrl.absoluteString
    }
}

enum ViewingMode: String, CaseIterable {
    case original
}

// MARK: - WebView (Original)

#if os(iOS)
    struct WebView: UIViewRepresentable {
        let url: URL

        func makeUIView(context: Context) -> WKWebView {
            WKWebView(frame: .zero)
        }

        func updateUIView(_ webView: WKWebView, context: Context) {
            if webView.url != url {
                webView.load(URLRequest(url: url))
            }
        }
    }
#endif

#if os(macOS)
    import AppKit

    struct WebView: NSViewRepresentable {
        let url: URL

        func makeNSView(context: Context) -> WKWebView {
            WKWebView(frame: .zero)
        }

        func updateNSView(_ webView: WKWebView, context: Context) {
            if webView.url != url {
                webView.load(URLRequest(url: url))
            }
        }
    }
#endif

// MARK: - Archive Opening / Extraction Helpers (iOS)

// MARK: - Toolbar Modifier

struct InboxToolbarModifier: ViewModifier {
    @Binding var sort: LinkSort
    @Binding var showFailedOnly: Bool
    let syncing: Bool
    let runSync: () async -> Void

    @EnvironmentObject var authManager: NativeAuthManager
    @EnvironmentObject var biometricManager: BiometricAuthManager

    func body(content: Content) -> some View {
        content
            #if os(iOS)
                .toolbar {
                    ToolbarItemGroup(placement: .navigationBarLeading) {
                        Menu {
                            Picker("Sort", selection: $sort) {
                                ForEach(LinkSort.allCases, id: \.self) { s in
                                    Text(s.title).tag(s)
                                }
                            }
                            Toggle(isOn: $showFailedOnly) {
                                Label("Show Failed Only", systemImage: "exclamationmark.triangle")
                            }
                        } label: {
                            Label("Sort & Filter", systemImage: "line.3.horizontal.decrease.circle")
                        }
                    }
                    ToolbarItemGroup(placement: .navigationBarTrailing) {
                        Button {
                            Task { await runSync() }
                        } label: {
                            if syncing {
                                ProgressView().controlSize(.small)
                            } else {
                                Label("Sync", systemImage: "arrow.clockwise")
                            }
                        }
                        .disabled(syncing)
                        NavigationLink {
                            SettingsView()
                            .environmentObject(authManager)
                            .environmentObject(biometricManager)
                        } label: {
                            Image(systemName: "gearshape")
                        }
                    }
                }
            #elseif os(macOS)
                .toolbar {
                    ToolbarItemGroup(placement: .primaryAction) {
                        Menu {
                            Picker("Sort", selection: $sort) {
                                ForEach(LinkSort.allCases, id: \.self) { s in
                                    Text(s.title).tag(s)
                                }
                            }
                            Toggle(isOn: $showFailedOnly) {
                                Label(
                                    "Show Failed Only", systemImage: "exclamationmark.triangle")
                            }
                        } label: {
                            Label(
                                "Sort & Filter",
                                systemImage: "line.3.horizontal.decrease.circle")
                        }
                        Button {
                            Task { await runSync() }
                        } label: {
                            if syncing {
                                ProgressView().controlSize(.small)
                            } else {
                                Label("Sync", systemImage: "arrow.clockwise")
                            }
                        }
                        .disabled(syncing)
                        NavigationLink {
                            SettingsView()
                            .environmentObject(authManager)
                            .environmentObject(biometricManager)
                        } label: {
                            Image(systemName: "gearshape")
                        }
                    }
                }
            #endif
    }
}

// MARK: - Previews

#if DEBUG
    struct InboxView_Previews: PreviewProvider {
        static var previews: some View {
            InboxView()
                .modelContainer(previewContainer)
        }

        static var previewContainer: ModelContainer = {
            let container = AppModelSchema.makeInMemoryContainer()

            let ctx = ModelContext(container)

            // Seed a few items
            let now = Date()
            for i in 0..<5 {
                let url = URL(string: "https://example.com/articles/\(i)")!
                let link = LinkEntity(
                    originalUrl: url,
                    canonicalUrl: url,
                    title: "Example Article \(i)",
                    snippet: "Lorem ipsum dolor sit amet.",
                    sharedAt: now,
                    status: i % 4 == 0 ? "failed" : (i % 3 == 0 ? "queued" : "synced"),
                    updatedAt: now.addingTimeInterval(TimeInterval(-i * 3600)),
                    createdAt: now.addingTimeInterval(TimeInterval(-i * 7200))
                )
                ctx.insert(link)

            }
            try? ctx.save()
            return container
        }()
    }
#endif
