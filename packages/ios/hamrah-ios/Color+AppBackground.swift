//
//  Color+AppBackground.swift
//  hamrahIOS
//
//  Cross–platform helper for a neutral container background color.
//  Replaces direct uses of Color(.systemBackground) so the code compiles
//  cleanly on macOS (where UIColor / systemBackground aren't available)
//  while still matching native system appearance on each platform.
//
//  Usage:
//      .background(Color.appBackground)
//
//  If you need a secondary grouped background later, you can extend this
//  file with additional computed properties (e.g. `appSecondaryBackground`).
//

import SwiftUI

public extension Color {
    /// A cross‑platform system background color that adapts to light/dark mode.
    ///
    /// iOS / tvOS / watchOS:
    ///     Uses `UIColor.systemBackground`
    ///
    /// macOS:
    ///     Uses `NSColor.windowBackgroundColor` (visually closest to a primary
    ///     content surface) falling back to a plain `.background` if unavailable.
    ///
    /// Fallback:
    ///     Defaults to `.white` (light) / `.black` (dark) if system colors cannot
    ///     be resolved for some reason (e.g. extremely old OS targets).
    static var appBackground: Color {
        #if canImport(UIKit)
        // Use SwiftUI's system background color (dynamic light/dark aware).
        return Color(.systemBackground)
        #elseif canImport(AppKit)
        // NSColor.windowBackgroundColor provides a neutral window content surface.
        return Color(NSColor.windowBackgroundColor)
        #else
        // Minimal fallback for any other (unlikely) platform.
        return Color("AppBackgroundFallback", bundle: nil) // Allow override via asset, else clear/white.
        #endif
    }

}

// MARK: - Optional Preview
#if DEBUG
#Preview {
    VStack(spacing: 16) {
        Text("Primary Surface")
            .padding()
            .background(Color.appBackground)
            .cornerRadius(12)

        RoundedRectangle(cornerRadius: 12)
            .fill(Color.appBackground)
            .frame(height: 60)
            .overlay(Text("Example Container").font(.caption))
    }
    .padding()
    .background(LinearGradient(
        colors: [.blue.opacity(0.15), .purple.opacity(0.15)],
        startPoint: .topLeading,
        endPoint: .bottomTrailing))
}
#endif
