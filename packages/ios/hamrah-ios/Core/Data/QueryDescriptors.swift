//
//  QueryDescriptors.swift
//  hamrah-ios
//
//  Optimized SwiftData query descriptors for better performance
//

import Foundation
import SwiftData

// MARK: - Link Query Descriptors

struct LinkQueryDescriptors {

    // MARK: - Basic Queries

    static func all(
        limit: Int = 50,
        sort: LinkSort = .recent
    ) -> FetchDescriptor<LinkEntity> {
        var descriptor = FetchDescriptor<LinkEntity>()
        descriptor.fetchLimit = limit
        descriptor.sortBy = sort.sortDescriptors
        return descriptor
    }

    static func recent(limit: Int = 20) -> FetchDescriptor<LinkEntity> {
        var descriptor = FetchDescriptor<LinkEntity>()
        descriptor.fetchLimit = limit
        descriptor.sortBy = [SortDescriptor(\.updatedAt, order: .reverse)]
        return descriptor
    }

    static func failed(limit: Int = 50) -> FetchDescriptor<LinkEntity> {
        var descriptor = FetchDescriptor<LinkEntity>(
            predicate: #Predicate<LinkEntity> { $0.status == "failed" }
        )
        descriptor.fetchLimit = limit
        descriptor.sortBy = [SortDescriptor(\.updatedAt, order: .reverse)]
        return descriptor
    }

    static func pending(limit: Int = 50) -> FetchDescriptor<LinkEntity> {
        var descriptor = FetchDescriptor<LinkEntity>(
            predicate: #Predicate<LinkEntity> {
                $0.status == "queued" || $0.status == "syncing"
            }
        )
        descriptor.fetchLimit = limit
        descriptor.sortBy = [SortDescriptor(\.updatedAt, order: .reverse)]
        return descriptor
    }

    // MARK: - Search Queries

    static func search(
        term: String,
        limit: Int = 50,
        sort: LinkSort = .recent
    ) -> FetchDescriptor<LinkEntity> {
        let predicate = #Predicate<LinkEntity> {
            ($0.title ?? "").localizedStandardContains(term)
                || $0.originalUrl.absoluteString.localizedStandardContains(term)
                || ($0.snippet ?? "").localizedStandardContains(term)
                || ($0.summaryShort ?? "").localizedStandardContains(term)
        }

        var descriptor = FetchDescriptor<LinkEntity>(predicate: predicate)
        descriptor.fetchLimit = limit
        descriptor.sortBy = sort.sortDescriptors
        return descriptor
    }

    static func byDomain(
        domain: String,
        limit: Int = 50
    ) -> FetchDescriptor<LinkEntity> {
        let predicate = #Predicate<LinkEntity> {
            $0.canonicalUrl.absoluteString.contains(domain)
        }

        var descriptor = FetchDescriptor<LinkEntity>(predicate: predicate)
        descriptor.fetchLimit = limit
        descriptor.sortBy = [SortDescriptor(\.updatedAt, order: .reverse)]
        return descriptor
    }

    static func byTag(
        tagName: String,
        limit: Int = 50
    ) -> FetchDescriptor<LinkEntity> {
        let predicate = #Predicate<LinkEntity> {
            $0.tags.contains { $0.name == tagName }
        }

        var descriptor = FetchDescriptor<LinkEntity>(predicate: predicate)
        descriptor.fetchLimit = limit
        descriptor.sortBy = [SortDescriptor(\.updatedAt, order: .reverse)]
        return descriptor
    }

    // MARK: - Date Range Queries

    static func createdBetween(
        startDate: Date,
        endDate: Date,
        limit: Int = 50
    ) -> FetchDescriptor<LinkEntity> {
        let predicate = #Predicate<LinkEntity> {
            $0.createdAt >= startDate && $0.createdAt <= endDate
        }

        var descriptor = FetchDescriptor<LinkEntity>(predicate: predicate)
        descriptor.fetchLimit = limit
        descriptor.sortBy = [SortDescriptor(\.createdAt, order: .reverse)]
        return descriptor
    }

    static func updatedSince(
        date: Date,
        limit: Int = 50
    ) -> FetchDescriptor<LinkEntity> {
        let predicate = #Predicate<LinkEntity> {
            $0.updatedAt >= date
        }

        var descriptor = FetchDescriptor<LinkEntity>(predicate: predicate)
        descriptor.fetchLimit = limit
        descriptor.sortBy = [SortDescriptor(\.updatedAt, order: .reverse)]
        return descriptor
    }

    // MARK: - Complex Queries

    static func filtered(
        searchTerm: String? = nil,
        status: String? = nil,
        tags: [String] = [],
        sort: LinkSort = .recent,
        limit: Int = 50
    ) -> FetchDescriptor<LinkEntity> {

        var predicates: [Predicate<LinkEntity>] = []

        // Search term predicate
        if let searchTerm = searchTerm, !searchTerm.isEmpty {
            let searchPredicate = #Predicate<LinkEntity> {
                ($0.title ?? "").localizedStandardContains(searchTerm)
                    || $0.originalUrl.absoluteString.localizedStandardContains(searchTerm)
                    || ($0.snippet ?? "").localizedStandardContains(searchTerm)
                    || ($0.summaryShort ?? "").localizedStandardContains(searchTerm)
            }
            predicates.append(searchPredicate)
        }

        // Status predicate
        if let status = status {
            let statusPredicate = #Predicate<LinkEntity> { $0.status == status }
            predicates.append(statusPredicate)
        }

        // Tags predicate
        if !tags.isEmpty {
            let tagsPredicate = #Predicate<LinkEntity> { link in
                tags.allSatisfy { tagName in
                    link.tags.contains { $0.name == tagName }
                }
            }
            predicates.append(tagsPredicate)
        }

        // Combine predicates with AND logic
        let finalPredicate: Predicate<LinkEntity>? = predicates.reduce(nil) { result, predicate in
            if let result = result {
                return #Predicate<LinkEntity> { link in
                    result.evaluate(link) && predicate.evaluate(link)
                }
            } else {
                return predicate
            }
        }

        var descriptor = FetchDescriptor<LinkEntity>(predicate: finalPredicate)
        descriptor.fetchLimit = limit
        descriptor.sortBy = sort.sortDescriptors
        return descriptor
    }
}

// MARK: - Link Sort Enum

enum LinkSort: String, CaseIterable {
    case recent = "recent"
    case title = "title"
    case domain = "domain"
    case created = "created"

    var title: String {
        switch self {
        case .recent: return "Most Recent"
        case .title: return "Title A-Z"
        case .domain: return "Domain"
        case .created: return "Date Created"
        }
    }

    var sortDescriptors: [SortDescriptor<LinkEntity>] {
        switch self {
        case .recent:
            return [SortDescriptor(\.updatedAt, order: .reverse)]
        case .title:
            return [SortDescriptor(\.title, order: .forward)]
        case .domain:
            return [SortDescriptor(\.canonicalUrl.absoluteString, order: .forward)]
        case .created:
            return [SortDescriptor(\.createdAt, order: .reverse)]
        }
    }
}

// MARK: - Tag Query Descriptors

struct TagQueryDescriptors {
    static func all() -> FetchDescriptor<TagEntity> {
        var descriptor = FetchDescriptor<TagEntity>()
        descriptor.sortBy = [SortDescriptor(\.name, order: .forward)]
        return descriptor
    }

    static func popular(limit: Int = 20) -> FetchDescriptor<TagEntity> {
        var descriptor = FetchDescriptor<TagEntity>()
        descriptor.fetchLimit = limit
        // Note: This would ideally sort by link count, but SwiftData doesn't support
        // computed properties in predicates yet, so we'll sort by name for now
        descriptor.sortBy = [SortDescriptor(\.name, order: .forward)]
        return descriptor
    }
}
