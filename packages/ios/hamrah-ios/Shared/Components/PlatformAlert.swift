//
//  PlatformAlert.swift
//  hamrah-ios
//
//  Platform-specific alert implementation for iOS and macOS
//

import SwiftUI

struct PlatformAlert: ViewModifier {
    @Binding var isPresented: Bool
    let title: String
    let message: String?
    let primaryButton: AlertButton?
    let secondaryButton: AlertButton?

    init(
        isPresented: Binding<Bool>,
        title: String,
        message: String? = nil,
        primaryButton: AlertButton? = nil,
        secondaryButton: AlertButton? = nil
    ) {
        self._isPresented = isPresented
        self.title = title
        self.message = message
        self.primaryButton = primaryButton
        self.secondaryButton = secondaryButton
    }

    func body(content: Content) -> some View {
        content
            .alert(title, isPresented: $isPresented) {
                if let primaryButton = primaryButton {
                    Button(
                        primaryButton.title, role: primaryButton.role, action: primaryButton.action)
                }
                if let secondaryButton = secondaryButton {
                    Button(
                        secondaryButton.title, role: secondaryButton.role,
                        action: secondaryButton.action)
                }
                if primaryButton == nil && secondaryButton == nil {
                    Button("OK") {}
                }
            } message: {
                if let message = message {
                    Text(message)
                }
            }
    }
}

struct AlertButton {
    let title: String
    let role: ButtonRole?
    let action: () -> Void

    init(_ title: String, role: ButtonRole? = nil, action: @escaping () -> Void = {}) {
        self.title = title
        self.role = role
        self.action = action
    }

    static func cancel(_ action: @escaping () -> Void = {}) -> AlertButton {
        AlertButton("Cancel", role: .cancel, action: action)
    }

    static func destructive(_ title: String, action: @escaping () -> Void) -> AlertButton {
        AlertButton(title, role: .destructive, action: action)
    }

    static func `default`(_ title: String, action: @escaping () -> Void = {}) -> AlertButton {
        AlertButton(title, role: nil, action: action)
    }
}

extension View {
    func platformAlert(
        isPresented: Binding<Bool>,
        title: String,
        message: String? = nil,
        primaryButton: AlertButton? = nil,
        secondaryButton: AlertButton? = nil
    ) -> some View {
        modifier(
            PlatformAlert(
                isPresented: isPresented,
                title: title,
                message: message,
                primaryButton: primaryButton,
                secondaryButton: secondaryButton
            ))
    }
}
