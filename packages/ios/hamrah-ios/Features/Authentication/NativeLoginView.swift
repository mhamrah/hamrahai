//
//  NativeLoginView.swift
//  hamrahIOS
//
//  Native login interface with Apple Sign-In, Google Sign-In, and Passkeys
//

import AuthenticationServices
import SwiftUI

struct NativeLoginView: View {
    @EnvironmentObject var authManager: NativeAuthManager
    @State private var email = ""
    @State private var showingPasskeyLogin = false
    @State private var showingEmailInput = false
    @State private var showingWebAuthnSignUp = false

    var body: some View {
        Group {
            #if os(macOS)
                // macOS: Scroll view to better handle resizable windows
                ScrollView {
                    content
                        .frame(maxWidth: 640)
                        .padding(.top, 40)
                        .padding(.bottom, 60)
                        .frame(maxWidth: .infinity)
                }
                .frame(minWidth: 500, minHeight: 650)
            #else
                content
            #endif
        }
        .sheet(isPresented: $showingEmailInput) {
            PasskeyEmailInputView { email in
                Task {
                    await authManager.signInWithPasskey(email: email)
                }
                showingEmailInput = false
            }
            #if os(macOS)
                .frame(minWidth: 420, minHeight: 400)
            #endif
        }
        .sheet(isPresented: $showingWebAuthnSignUp) {
            WebAuthnSignUpView()
                .environmentObject(authManager)
                #if os(macOS)
                    .frame(minWidth: 520, minHeight: 600)
                #endif
        }
    }

    /// Extracted shared content so we can wrap differently per platform
    private var content: some View {
        VStack(spacing: 32) {
            Spacer(minLength: 0)

            // App Logo/Title
            VStack(spacing: 20) {
                Image(systemName: "person.crop.circle.fill")
                    .font(.system(size: 80))
                    .foregroundColor(.accentColor)

                Text("Welcome to Hamrah")
                    .font(.largeTitle)
                    .fontWeight(.bold)
                    .multilineTextAlignment(.center)

                Text("Sign in to continue")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
            }

            // Authentication Methods
            VStack(spacing: 16) {

                // Apple Sign-In
                Button(action: {
                    Task {
                        await authManager.signInWithApple()
                    }
                }) {
                    authButtonLabel(
                        symbol: "applelogo",
                        title: "Sign in with Apple",
                        background: Color.black
                    )
                }
                .disabled(authManager.isLoading)
                .accessibilityIdentifier("appleSignInButton")

                // Google Sign-In (may be stubbed if SDK absent)
                // Google Sign-In (only when Google SDK / flag is available)
                Button(action: {
                    Task {
                        await authManager.signInWithGoogle()
                    }
                }) {
                    authButtonLabel(
                        symbol: "globe",
                        title: "Continue with Google",
                        background: Color.blue
                    )
                }
                .disabled(authManager.isLoading)
                .accessibilityIdentifier("googleSignInButton")

                // Passkey Sign-In
                Button(action: { showingEmailInput = true }) {
                    authButtonLabel(
                        symbol: "key",
                        title: "Sign in with Email + Passkey",
                        background: .purple
                    )
                }
                .disabled(authManager.isLoading)
                .accessibilityIdentifier("passkeySignInButton")
            }

            // Loading Indicator
            if authManager.isLoading {
                HStack {
                    ProgressView()
                        .scaleEffect(0.8)
                    Text("Signing in...")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                .padding(.top, 8)
            }

            // Error Message
            if let errorMessage = authManager.errorMessage {
                Text(errorMessage)
                    .font(.caption)
                    .foregroundColor(.red)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal)
                    .padding(.top, 8)
                    .transition(.opacity)
            }

            // Footer with Sign Up Option
            VStack(spacing: 12) {
                Button("Create Account with Passkey") {
                    showingWebAuthnSignUp = true
                }
                .font(.subheadline)
                .foregroundColor(.purple)
                .disabled(authManager.isLoading)
                .accessibilityIdentifier("createPasskeyAccountButton")

                VStack(spacing: 8) {
                    Text("Secure authentication")
                        .font(.caption)
                        .foregroundColor(.secondary)

                    Text("Your data is protected and synced across devices")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: 320)
                }
            }

            Spacer(minLength: 0)
        }
        .padding(.horizontal, platformHorizontalPadding)
        .padding(.vertical, platformVerticalPadding)
    }

    private func authButtonLabel(symbol: String, title: String, background: Color) -> some View {
        HStack {
            Image(systemName: symbol)
                .foregroundColor(.white)
            Text(title)
                .font(.headline)
                .foregroundColor(.white)
                .lineLimit(1)
                .minimumScaleFactor(0.8)
        }
        .frame(maxWidth: .infinity)
        .frame(height: 50)
        .background(background)
        .cornerRadius(8)
        #if os(macOS)
            .buttonStyle(.plain)
        #endif
    }

    private var platformHorizontalPadding: CGFloat {
        #if os(macOS)
            return 40
        #else
            return 32
        #endif
    }

    private var platformVerticalPadding: CGFloat {
        #if os(macOS)
            return 30
        #else
            return 20
        #endif
    }
}

struct PasskeyEmailInputView: View {
    @State private var email = ""
    @Environment(\.presentationMode) var presentationMode
    let onContinue: (String) -> Void

    var body: some View {
        #if os(macOS)
            // NavigationView on macOS is acceptable, but provide sizing hints
            NavigationView {
                inner
                    .frame(minWidth: 420, minHeight: 420)
            }
        #else
            NavigationView { inner }
        #endif
    }

    private var inner: some View {
        VStack(spacing: 24) {
            VStack(spacing: 16) {
                Image(systemName: "faceid")
                    .font(.system(size: 60))
                    .foregroundColor(.purple)

                Text("Sign in with Passkey")
                    .font(.title2)
                    .fontWeight(.semibold)

                Text("Enter your email to use your saved passkey")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
            }

            VStack(spacing: 16) {
                TextField("Email address", text: $email)
                    .textFieldStyle(RoundedBorderTextFieldStyle())
                    #if os(iOS)
                        .keyboardType(.emailAddress)
                        .autocapitalization(.none)
                        .disableAutocorrection(true)
                    #endif

                Button("Continue with Passkey") {
                    onContinue(email)
                }
                .font(.headline)
                .foregroundColor(.white)
                .frame(maxWidth: .infinity)
                .frame(height: 50)
                .background(email.isEmpty ? Color.gray : Color.purple)
                .cornerRadius(8)
                .disabled(email.isEmpty)
            }

            Spacer()
        }
        .padding(24)
        .navigationTitle("Passkey Sign-In")
        #if os(iOS)
            .navigationBarTitleDisplayMode(.inline)
            .navigationBarItems(
                leading: Button("Cancel") {
                    presentationMode.wrappedValue.dismiss()
                }
            )
        #else
            .toolbar {
                ToolbarItem(placement: .navigation) {
                    Button("Close") {
                        presentationMode.wrappedValue.dismiss()
                    }
                }
            }
        #endif
    }
}

#Preview {
    NativeLoginView()
        .environmentObject(NativeAuthManager())
}
