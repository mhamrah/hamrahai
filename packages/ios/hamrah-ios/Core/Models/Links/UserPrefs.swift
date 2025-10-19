import Foundation
import SwiftData

@Model
final class UserPrefs {
    // User-scoped preferences (not device-specific)
    @Attribute(.unique) var id: UUID = UUID(uuidString: "00000000-0000-0000-0000-000000000001")!

    var defaultModel: String = "gpt-4o-mini"
    var preferredModelsJSON: Data = Data()
    var lastUpdatedAt: Date = Date()

    @Transient
    var preferredModels: [String] {
        get {
            if preferredModelsJSON.isEmpty {
                return []
            }
            do {
                return try JSONDecoder().decode([String].self, from: preferredModelsJSON)
            } catch {
                print("Error decoding preferredModels: \(error)")
                return []
            }
        }
        set {
            do {
                preferredModelsJSON = try JSONEncoder().encode(newValue)
            } catch {
                print("Error encoding preferredModels: \(error)")
            }
        }
    }

    init(
        defaultModel: String = "gpt-4o-mini",
        preferredModels: [String] = [],
        lastUpdatedAt: Date = Date()
    ) {
        self.defaultModel = defaultModel
        self.lastUpdatedAt = lastUpdatedAt
        self.preferredModels = preferredModels
    }
}
