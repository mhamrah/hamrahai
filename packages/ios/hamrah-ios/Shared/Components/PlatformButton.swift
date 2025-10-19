//
//  PlatformButton.swift
//  hamrah-ios
//
//  Platform-specific button implementation for iOS and macOS
//

import SwiftUI

struct PlatformButton: View {
    let title: String
    let systemImage: String?
    let style: PlatformButtonStyle
    let action: () -> Void

    init(
        _ title: String,
        systemImage: String? = nil,
        style: PlatformButtonStyle = .primary,
        action: @escaping () -> Void
    ) {
        self.title = title
        self.systemImage = systemImage
        self.style = style
        self.action = action
    }

    var body: some View {
        applyButtonStyle()
    }

    @ViewBuilder
    private var buttonContent: some View {
        Button(action: action) {
            HStack(spacing: Theme.Spacing.small) {
                if let systemImage = systemImage {
                    Image(systemName: systemImage)
                }
                Text(title)
            }
        }
    }

    @ViewBuilder
    private func applyButtonStyle() -> some View {
        #if os(iOS)
            switch style {
            case .primary:
                buttonContent.buttonStyle(.borderedProminent)
            case .secondary:
                buttonContent.buttonStyle(.bordered)
            case .destructive:
                buttonContent.buttonStyle(DestructiveButtonStyle())
            case .plain:
                buttonContent.buttonStyle(.plain)
            }
        #elseif os(macOS)
            switch style {
            case .primary:
                buttonContent.buttonStyle(.borderedProminent)
            case .secondary:
                buttonContent.buttonStyle(.bordered)
            case .destructive:
                buttonContent.buttonStyle(MacOSDestructiveButtonStyle())
            case .plain:
                buttonContent.buttonStyle(.plain)
            }
        #endif
    }
}

enum PlatformButtonStyle {
    case primary
    case secondary
    case destructive
    case plain
}

// MARK: - Platform-specific button styles

#if os(iOS)
    struct DestructiveButtonStyle: ButtonStyle {
        func makeBody(configuration: Configuration) -> some View {
            configuration.label
                .foregroundColor(.white)
                .padding(.horizontal, Theme.Spacing.medium)
                .padding(.vertical, Theme.Spacing.small)
                .background(Color.red)
                .cornerRadius(Theme.CornerRadius.button)
                .scaleEffect(configuration.isPressed ? 0.95 : 1.0)
        }
    }
#elseif os(macOS)
    struct MacOSDestructiveButtonStyle: ButtonStyle {
        func makeBody(configuration: Configuration) -> some View {
            configuration.label
                .foregroundColor(.white)
                .padding(.horizontal, Theme.Spacing.medium)
                .padding(.vertical, Theme.Spacing.small)
                .background(Color.red)
                .cornerRadius(Theme.CornerRadius.button)
                .opacity(configuration.isPressed ? 0.8 : 1.0)
        }
    }
#endif

#Preview {
    VStack(spacing: 16) {
        PlatformButton("Primary Button", systemImage: "star.fill", style: .primary) {}
        PlatformButton("Secondary Button", style: .secondary) {}
        PlatformButton("Destructive Button", style: .destructive) {}
        PlatformButton("Plain Button", style: .plain) {}
    }
    .padding()
}
