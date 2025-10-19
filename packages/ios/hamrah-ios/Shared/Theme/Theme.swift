//
//  Theme.swift
//  hamrah-ios
//
//  Centralized theme system for consistent styling across the app
//

import SwiftUI

struct Theme {

    // MARK: - Colors

    struct Colors {
        // Brand color palette
        static let hunyadiYellow = Color(hex: "f6bd60")
        static let linen = Color(hex: "f7ede2")
        static let teaRose = Color(hex: "f5cac3")
        static let cambridgeBlue = Color(hex: "84a59d")
        static let lightCoral = Color(hex: "f28482")

        // Primary colors (mapped to brand palette)
        static let primary = hunyadiYellow
        static let secondary = cambridgeBlue
        static let tertiary = teaRose

        // Background colors
        #if os(iOS)
            static let background = Color(.systemBackground)
            static let secondaryBackground = Color(.secondarySystemBackground)
            static let tertiaryBackground = Color(.tertiarySystemBackground)
        #elseif os(macOS)
            static let background = Color(NSColor.windowBackgroundColor)
            static let secondaryBackground = Color(NSColor.controlBackgroundColor)
            static let tertiaryBackground = Color(NSColor.underPageBackgroundColor)
        #endif

        // Card and surface colors
        #if os(iOS)
            static let cardBackground = Color(.systemBackground)
            static let surfaceBackground = Color(.systemGroupedBackground)
        #elseif os(macOS)
            static let cardBackground = Color(NSColor.windowBackgroundColor)
            static let surfaceBackground = Color(NSColor.controlBackgroundColor)
        #endif

        // Status colors (using brand palette)
        static let success = cambridgeBlue
        static let warning = hunyadiYellow
        static let error = lightCoral
        static let info = cambridgeBlue

        // Link status colors
        static let linkQueued = Color.yellow
        static let linkSyncing = Color.blue
        static let linkSynced = Color.green
        static let linkFailed = Color.red

        // Text colors
        static let primaryText = Color.primary
        static let secondaryText = Color.secondary
        #if os(iOS)
            static let tertiaryText = Color(.tertiaryLabel)
        #elseif os(macOS)
            static let tertiaryText = Color(NSColor.tertiaryLabelColor)
        #endif

        // Border colors
        #if os(iOS)
            static let border = Color(.separator)
            static let secondaryBorder = Color(.opaqueSeparator)
        #elseif os(macOS)
            static let border = Color(NSColor.separatorColor)
            static let secondaryBorder = Color(NSColor.gridColor)
        #endif

        static func linkStatusColor(_ status: String) -> Color {
            switch status {
            case "queued": return linkQueued
            case "syncing": return linkSyncing
            case "synced": return linkSynced
            case "failed": return linkFailed
            default: return Color.gray
            }
        }
    }

    // MARK: - Typography

    struct Typography {
        static let largeTitle = Font.largeTitle.weight(.bold)
        static let title = Font.title.weight(.semibold)
        static let title2 = Font.title2.weight(.semibold)
        static let title3 = Font.title3.weight(.medium)
        static let headline = Font.headline
        static let subheadline = Font.subheadline
        static let body = Font.body
        static let callout = Font.callout
        static let footnote = Font.footnote
        static let caption = Font.caption
        static let caption2 = Font.caption2

        // Custom fonts for specific use cases
        static let cardTitle = Font.headline.weight(.medium)
        static let cardSubtitle = Font.subheadline.weight(.regular)
        static let buttonText = Font.body.weight(.medium)
        static let navigationTitle = Font.title2.weight(.semibold)
    }

    // MARK: - Spacing

    struct Spacing {
        static let xsmall: CGFloat = 4
        static let small: CGFloat = 8
        static let medium: CGFloat = 16
        static let large: CGFloat = 24
        static let xlarge: CGFloat = 32
        static let xxlarge: CGFloat = 48

        // Semantic spacing
        static let cardPadding: CGFloat = medium
        static let screenPadding: CGFloat = medium
        static let sectionSpacing: CGFloat = large
        static let itemSpacing: CGFloat = small
    }

    // MARK: - Corner Radius

    struct CornerRadius {
        static let small: CGFloat = 6
        static let medium: CGFloat = 8
        static let large: CGFloat = 12
        static let xlarge: CGFloat = 16

        // Semantic corner radius
        static let card: CGFloat = large
        static let button: CGFloat = medium
        static let textField: CGFloat = medium
        static let image: CGFloat = small
    }

    // MARK: - Shadows

    struct Shadow {
        static let card = ShadowStyle(
            color: Color.black.opacity(0.1),
            radius: 4,
            x: 0,
            y: 2
        )

        static let button = ShadowStyle(
            color: Color.black.opacity(0.1),
            radius: 2,
            x: 0,
            y: 1
        )

        static let modal = ShadowStyle(
            color: Color.black.opacity(0.3),
            radius: 10,
            x: 0,
            y: 5
        )
    }

    // MARK: - Animation

    struct Animation {
        static let quick = SwiftUI.Animation.easeInOut(duration: 0.2)
        static let standard = SwiftUI.Animation.easeInOut(duration: 0.3)
        static let slow = SwiftUI.Animation.easeInOut(duration: 0.5)

        // Semantic animations
        static let buttonPress = quick
        static let viewTransition = standard
        static let modalPresentation = slow
    }

    // MARK: - Icons

    struct Icons {
        // Navigation
        static let settings = "gearshape"
        static let back = "chevron.left"
        static let close = "xmark"

        // Actions
        static let add = "plus"
        static let edit = "pencil"
        static let delete = "trash"
        static let share = "square.and.arrow.up"
        static let save = "bookmark"
        static let copy = "doc.on.doc"

        // Status
        static let success = "checkmark.circle.fill"
        static let warning = "exclamationmark.triangle.fill"
        static let error = "xmark.circle.fill"
        static let info = "info.circle.fill"

        // Content
        static let link = "link"
        static let web = "globe"

        static let tag = "tag"
        static let search = "magnifyingglass"
        static let filter = "line.3.horizontal.decrease.circle"

        // Authentication
        static let faceID = "faceid"
        static let touchID = "touchid"
        static let apple = "apple.logo"
        static let google = "g.circle"
    }
}

// MARK: - Shadow Style Helper

struct ShadowStyle {
    let color: Color
    let radius: CGFloat
    let x: CGFloat
    let y: CGFloat
}

// MARK: - Color Extension for Hex Support

extension Color {
    init(hex: String) {
        let hex = hex.trimmingCharacters(in: CharacterSet.alphanumerics.inverted)
        var int: UInt64 = 0
        Scanner(string: hex).scanHexInt64(&int)
        let a: UInt64
        let r: UInt64
        let g: UInt64
        let b: UInt64
        switch hex.count {
        case 3:  // RGB (12-bit)
            (a, r, g, b) = (255, (int >> 8) * 17, (int >> 4 & 0xF) * 17, (int & 0xF) * 17)
        case 6:  // RGB (24-bit)
            (a, r, g, b) = (255, int >> 16, int >> 8 & 0xFF, int & 0xFF)
        case 8:  // ARGB (32-bit)
            (a, r, g, b) = (int >> 24, int >> 16 & 0xFF, int >> 8 & 0xFF, int & 0xFF)
        default:
            (a, r, g, b) = (255, 0, 0, 0)
        }

        self.init(
            .sRGB,
            red: Double(r) / 255,
            green: Double(g) / 255,
            blue: Double(b) / 255,
            opacity: Double(a) / 255
        )
    }
}

// MARK: - View Extensions for Theme

extension View {
    func themedCard(padding: CGFloat = Theme.Spacing.cardPadding) -> some View {
        self
            .padding(padding)
            .background(Theme.Colors.cardBackground)
            .cornerRadius(Theme.CornerRadius.card)
            .shadow(
                color: Theme.Shadow.card.color,
                radius: Theme.Shadow.card.radius,
                x: Theme.Shadow.card.x,
                y: Theme.Shadow.card.y
            )
    }

    func themedButton() -> some View {
        self
            .font(Theme.Typography.buttonText)
            .cornerRadius(Theme.CornerRadius.button)
    }

    func themedSection(spacing: CGFloat = Theme.Spacing.sectionSpacing) -> some View {
        self
            .padding(.vertical, spacing)
    }
}
