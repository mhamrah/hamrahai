//
//  PlatformTextField.swift
//  hamrah-ios
//
//  Platform-specific text field implementation for iOS and macOS
//

import SwiftUI

struct PlatformTextField: View {
    let title: String
    @Binding var text: String
    let placeholder: String
    let isSecure: Bool
    let keyboardType: PlatformKeyboardType

    init(
        _ title: String,
        text: Binding<String>,
        placeholder: String = "",
        isSecure: Bool = false,
        keyboardType: PlatformKeyboardType = .default
    ) {
        self.title = title
        self._text = text
        self.placeholder = placeholder
        self.isSecure = isSecure
        self.keyboardType = keyboardType
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.Spacing.xsmall) {
            if !title.isEmpty {
                Text(title)
                    .font(.headline)
                    .foregroundColor(Theme.Colors.primary)
            }

            Group {
                if isSecure {
                    SecureField(placeholder, text: $text)
                } else {
                    TextField(placeholder, text: $text)
                        #if os(iOS)
                            .keyboardType(keyboardType.uiKeyboardType)
                        #endif
                }
            }
            .textFieldStyle(platformTextFieldStyle)
        }
    }

    private var platformTextFieldStyle: RoundedBorderTextFieldStyle {
        RoundedBorderTextFieldStyle()
    }
}

enum PlatformKeyboardType {
    case `default`
    case emailAddress
    case URL
    case numberPad
    case phonePad

    #if os(iOS)
        var uiKeyboardType: UIKeyboardType {
            switch self {
            case .default: return .default
            case .emailAddress: return .emailAddress
            case .URL: return .URL
            case .numberPad: return .numberPad
            case .phonePad: return .phonePad
            }
        }
    #endif
}

#Preview {
    VStack(spacing: 16) {
        PlatformTextField(
            "Email", text: .constant(""), placeholder: "Enter your email",
            keyboardType: .emailAddress)
        PlatformTextField(
            "Password", text: .constant(""), placeholder: "Enter password", isSecure: true)
        PlatformTextField(
            "URL", text: .constant(""), placeholder: "https://example.com", keyboardType: .URL)
    }
    .padding()
}
