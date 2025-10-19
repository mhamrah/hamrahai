//
//  LinkCard.swift
//  hamrah-ios
//
//  Enhanced link card component with rich preview
//

import SwiftUI

struct LinkCard: View {
    let link: LinkEntity
    let onTap: () -> Void
    let onOpenOriginal: () -> Void

    @State private var showingContextMenu = false

    // Normalize optional/non-optional tags to a concrete array
    private var tagsArray: [TagEntity] {
        link.tags
    }

    var body: some View {
        Button(action: onTap) {
            VStack(alignment: .leading, spacing: Theme.Spacing.small) {
                // Header with favicon, domain, and status
                HStack(spacing: Theme.Spacing.small) {
                    // Favicon placeholder (could be enhanced with real favicon loading)
                    Image(systemName: Theme.Icons.web)
                        .font(.caption)
                        .foregroundColor(Theme.Colors.secondaryText)
                        .frame(width: 16, height: 16)

                    Text(domain)
                        .font(Theme.Typography.caption)
                        .foregroundColor(Theme.Colors.secondaryText)
                        .lineLimit(1)

                    Spacer()

                    HStack(spacing: Theme.Spacing.xsmall) {
                        StatusIndicator(status: link.status)

                        if !tagsArray.isEmpty {
                            Image(systemName: Theme.Icons.tag)
                                .font(.caption2)
                                .foregroundColor(Theme.Colors.primary)
                        }
                    }
                }

                // Title
                Text(link.title ?? "Untitled")
                    .font(Theme.Typography.cardTitle)
                    .foregroundColor(Theme.Colors.primaryText)
                    .lineLimit(2)
                    .multilineTextAlignment(.leading)

                // Summary or snippet
                if let summary = link.summaryShort ?? link.snippet, !summary.isEmpty {
                    Text(summary)
                        .font(Theme.Typography.cardSubtitle)
                        .foregroundColor(Theme.Colors.secondaryText)
                        .lineLimit(3)
                        .multilineTextAlignment(.leading)
                }

                // Tags
                if !tagsArray.isEmpty {
                    TagCloud(tags: tagsArray.prefix(3).map { $0.name })
                }

                // Footer with metadata
                HStack {
                    if let date = relativeDate(link.updatedAt) {
                        Text(date)
                            .font(Theme.Typography.caption2)
                            .foregroundColor(Theme.Colors.tertiaryText)
                    }

                    Spacer()

                    if link.saveCount > 1 {
                        HStack(spacing: 2) {
                            Image(systemName: "bookmark.fill")
                            Text("\(link.saveCount)")
                        }
                        .font(Theme.Typography.caption2)
                        .foregroundColor(Theme.Colors.primary)
                    }
                }
            }
            .padding(Theme.Spacing.medium)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .buttonStyle(PlainButtonStyle())
        .themedCard(padding: 0)
        .contextMenu {
            contextMenuContent
        }
    }

    // MARK: - Computed Properties

    private var domain: String {
        link.canonicalUrl.host ?? link.originalUrl.host ?? "Unknown"
    }

    @ViewBuilder
    private var contextMenuContent: some View {
        Button {
            onOpenOriginal()
        } label: {
            Label("Open Original", systemImage: Theme.Icons.web)
        }

        Button {
            copyURL()
        } label: {
            Label("Copy URL", systemImage: Theme.Icons.copy)
        }

        Divider()

        if link.status == "failed" {
            Button {
                retrySync()
            } label: {
                Label("Retry Sync", systemImage: "arrow.clockwise")
            }
        }

        Button(role: .destructive) {
            deleteLink()
        } label: {
            Label("Delete", systemImage: Theme.Icons.delete)
        }
    }

    // MARK: - Helper Methods

    private func relativeDate(_ date: Date) -> String? {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: date, relativeTo: Date())
    }

    private func copyURL() {
        PlatformBridge.copyToClipboard(link.canonicalUrl.absoluteString)
    }

    private func retrySync() {
        // This would typically be handled by the parent view model
        NotificationCenter.default.post(
            name: .retryLinkSync,
            object: link
        )
    }

    private func deleteLink() {
        // This would typically be handled by the parent view model
        NotificationCenter.default.post(
            name: .deleteLinkRequest,
            object: link
        )
    }
}

// MARK: - Supporting Components

struct StatusIndicator: View {
    let status: String

    var body: some View {
        Circle()
            .fill(Theme.Colors.linkStatusColor(status))
            .frame(width: 8, height: 8)
            .accessibilityLabel(statusAccessibilityLabel)
    }

    private var statusAccessibilityLabel: String {
        switch status {
        case "queued": return "Queued for sync"
        case "syncing": return "Syncing"
        case "synced": return "Synced"
        case "failed": return "Sync failed"
        default: return "Unknown status"
        }
    }
}

struct TagCloud: View {
    let tags: [String]

    var body: some View {
        HStack(spacing: Theme.Spacing.xsmall) {
            ForEach(tags, id: \.self) { tag in
                Text(tag)
                    .font(Theme.Typography.caption2)
                    .padding(.horizontal, Theme.Spacing.small)
                    .padding(.vertical, 2)
                    .background(Theme.Colors.primary.opacity(0.1))
                    .foregroundColor(Theme.Colors.primary)
                    .cornerRadius(Theme.CornerRadius.small)
            }
        }
    }
}

// MARK: - Notification Names

extension Notification.Name {
    static let retryLinkSync = Notification.Name("retryLinkSync")
    static let deleteLinkRequest = Notification.Name("deleteLinkRequest")
}

// MARK: - Preview

#Preview {
    let sampleLink = LinkEntity(
        originalUrl: URL(string: "https://example.com/article")!,
        canonicalUrl: URL(string: "https://example.com/article")!,
        title: "Sample Article Title That Might Be Long",
        snippet:
            "This is a sample snippet that provides a preview of the article content. It might be quite long and should be truncated appropriately.",
        status: "synced"
    )

    return VStack(spacing: 16) {
        LinkCard(
            link: sampleLink,
            onTap: {},
            onOpenOriginal: {}
        )

        LinkCard(
            link: LinkEntity(
                originalUrl: URL(string: "https://failed.com")!,
                canonicalUrl: URL(string: "https://failed.com")!,
                title: "Failed Link",
                status: "failed"
            ),
            onTap: {},
            onOpenOriginal: {}
        )
    }
    .padding()
}
