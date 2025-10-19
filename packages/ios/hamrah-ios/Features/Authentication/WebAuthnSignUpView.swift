//
//  WebAuthnSignUpView.swift
//  hamrahIOS
//
//  DEPRECATED: This view previously attempted a WebAuthn-only account creation flow.
//  The current backend requires an existing authenticated user (via OAuth or another
//  supported method) before a passkey can be registered. This UI is retained for
//  potential future use, but it is not functional in the present architecture.
//  If invoked, it should be hidden or replaced by an OAuth-first onboarding flow.
//
//  WebAuthn-only sign up flow for creating new accounts with passkeys (inactive)
//

#if os(iOS)
    import AuthenticationServices
    import SwiftUI

    struct WebAuthnSignUpView: View {
        @Environment(\.dismiss) private var dismiss
        @EnvironmentObject var authManager: NativeAuthManager
        @State private var email = ""
        @State private var name = ""
        @State private var isLoading = false
        @State private var errorMessage: String?

        var body: some View {
            NavigationView {
                VStack(spacing: 32) {
                    Spacer()

                    // Header
                    VStack(spacing: 20) {
                        Image(systemName: "key.fill")
                            .font(.system(size: 60))
                            .foregroundColor(.purple)

                        Text("Create Account with Passkey")
                            .font(.title)
                            .fontWeight(.bold)
                            .multilineTextAlignment(.center)

                        Text("Create a secure account using your device's biometric authentication")
                            .font(.subheadline)
                            .foregroundColor(.secondary)
                            .multilineTextAlignment(.center)
                            .padding(.horizontal)
                    }

                    Spacer()

                    // Form Fields
                    VStack(spacing: 20) {
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Full Name")
                                .font(.headline)
                                .foregroundColor(.primary)

                            TextField("Enter your full name", text: $name)
                                .textFieldStyle(RoundedBorderTextFieldStyle())
                                .autocapitalization(.words)
                                .disableAutocorrection(true)
                        }

                        VStack(alignment: .leading, spacing: 8) {
                            Text("Email Address")
                                .font(.headline)
                                .foregroundColor(.primary)

                            TextField("Enter your email", text: $email)
                                .textFieldStyle(RoundedBorderTextFieldStyle())
                                .keyboardType(.emailAddress)
                                .autocapitalization(.none)
                                .disableAutocorrection(true)
                        }
                    }

                    // Sign Up Button
                    Button(action: signUpWithWebAuthn) {
                        HStack {
                            if isLoading {
                                ProgressView()
                                    .scaleEffect(0.8)
                                    .tint(.white)
                            } else {
                                Image(systemName: "faceid")
                                    .font(.title3)
                            }
                            Text(isLoading ? "Creating Account..." : "Create Account with Passkey")
                                .fontWeight(.semibold)
                        }
                        .foregroundColor(.white)
                        .frame(maxWidth: .infinity)
                        .frame(height: 50)
                        .background(canSignUp ? Color.purple : Color.gray)
                        .cornerRadius(8)
                    }
                    .disabled(!canSignUp || isLoading)

                    // Error Message
                    if let errorMessage = errorMessage {
                        Text(errorMessage)
                            .font(.caption)
                            .foregroundColor(.red)
                            .multilineTextAlignment(.center)
                            .padding(.horizontal)
                    }

                    Spacer()

                    // Footer
                    VStack(spacing: 8) {
                        Text("Your passkey will be securely stored on this device")
                            .font(.caption)
                            .foregroundColor(.secondary)
                            .multilineTextAlignment(.center)

                        Text("No passwords required")
                            .font(.caption2)
                            .foregroundColor(.secondary)
                    }
                }
                .padding(.horizontal, 32)
                .padding(.vertical, 20)
                .navigationTitle("Sign Up")
                #if os(iOS)
                    .navigationBarTitleDisplayMode(.inline)
                    .navigationBarItems(
                        leading: Button("Cancel") {
                            dismiss()
                        }.disabled(isLoading)
                    )
                #else
                    .toolbar {
                        ToolbarItem(placement: .cancellationAction) {
                            Button("Cancel") {
                                dismiss()
                            }.disabled(isLoading)
                        }
                    }
                #endif
            }
        }

        private var canSignUp: Bool {
            !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                && !email.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                && email.contains("@")
        }

        private func signUpWithWebAuthn() {
            guard canSignUp else { return }

            isLoading = true
            errorMessage = nil

            Task {
                do {
                    try await registerNewUserWithPasskey(
                        email: email.trimmingCharacters(in: .whitespacesAndNewlines),
                        name: name.trimmingCharacters(in: .whitespacesAndNewlines))

                    await MainActor.run {
                        dismiss()
                    }
                } catch {
                    await MainActor.run {
                        self.errorMessage = error.localizedDescription
                        self.isLoading = false
                    }
                }
            }
        }

        private func registerNewUserWithPasskey(email: String, name: String) async throws {
            // Step 1: Begin WebAuthn registration for new user
            let beginOptions = try await beginWebAuthnRegistrationForNewUser(
                email: email, name: name)

            guard let options = beginOptions.options else {
                throw NSError(
                    domain: "WebAuthn", code: -1,
                    userInfo: [NSLocalizedDescriptionKey: "No registration options received"])
            }

            let challengeId = options.challengeId

            // Step 2: Perform platform registration
            let attestation = try await performPlatformRegistration(options: options)

            // Step 3: Complete registration with backend
            try await completeWebAuthnRegistrationForNewUser(
                attestation: attestation,
                challengeId: challengeId,
                email: email,
                name: name
            )
        }

        private func beginWebAuthnRegistrationForNewUser(email: String, name: String) async throws
            -> WebAuthnBeginRegistrationResponse
        {
            let body = [
                "email": email,
                "name": name,
            ]

            return try await SecureAPIService.shared.post(
                endpoint: "/api/webauthn/register/begin",
                body: body,
                accessToken: nil,  // No auth needed for new user registration
                responseType: WebAuthnBeginRegistrationResponse.self,
                customBaseURL: APIConfiguration.shared.baseURL
            )
        }

        private func performPlatformRegistration(options: PublicKeyCredentialCreationOptions)
            async throws -> ASAuthorizationPlatformPublicKeyCredentialRegistration
        {
            let challenge = Data(base64Encoded: options.challenge) ?? Data()
            let userID = Data(base64Encoded: options.user.id) ?? Data()

            let request = ASAuthorizationPlatformPublicKeyCredentialProvider(
                relyingPartyIdentifier: options.rp.id
            )
            .createCredentialRegistrationRequest(
                challenge: challenge, name: options.user.name, userID: userID)

            return try await withCheckedThrowingContinuation { continuation in
                let controller = ASAuthorizationController(authorizationRequests: [request])

                // Store continuation for delegate callback
                NewUserPasskeyRegistrationDelegate.shared.setContinuation(continuation)

                controller.delegate = NewUserPasskeyRegistrationDelegate.shared
                controller.presentationContextProvider = authManager
                controller.performRequests()
            }
        }

        private func completeWebAuthnRegistrationForNewUser(
            attestation: ASAuthorizationPlatformPublicKeyCredentialRegistration,
            challengeId: String,
            email: String, name: String
        ) async throws {
            // Create the response object matching SimpleWebAuthn's RegistrationResponseJSON format
            let registrationResponseData =
                [
                    "id": attestation.credentialID.base64EncodedString(),
                    "rawId": attestation.credentialID.base64EncodedString(),
                    "type": "public-key",
                    "response": [
                        "attestationObject": attestation.rawAttestationObject?.base64EncodedString()
                            ?? "",
                        "clientDataJSON": attestation.rawClientDataJSON.base64EncodedString(),
                    ],
                ] as [String: Any]

            let body =
                [
                    "response": registrationResponseData,
                    "challengeId": challengeId,
                    "email": email,
                    "name": name,
                ] as [String: Any]

            let result = try await SecureAPIService.shared.post(
                endpoint: "/api/webauthn/register/verify",
                body: body,
                accessToken: nil,  // No auth needed for new user registration
                responseType: APIResponse.self,
                customBaseURL: APIConfiguration.shared.baseURL
            )

            // If successful, the user should now be signed in
            if result.success {
                // Trigger a sign-in to get the session
                await authManager.signInWithPasskey(email: email)
            }
        }
    }

    // MARK: - New User Passkey Registration Delegate

    class NewUserPasskeyRegistrationDelegate: NSObject, ASAuthorizationControllerDelegate {
        static let shared = NewUserPasskeyRegistrationDelegate()

        private var continuation:
            CheckedContinuation<ASAuthorizationPlatformPublicKeyCredentialRegistration, Error>?

        func setContinuation(
            _ continuation: CheckedContinuation<
                ASAuthorizationPlatformPublicKeyCredentialRegistration, Error
            >
        ) {
            self.continuation = continuation
        }

        func authorizationController(
            controller: ASAuthorizationController,
            didCompleteWithAuthorization authorization: ASAuthorization
        ) {
            if let registration = authorization.credential
                as? ASAuthorizationPlatformPublicKeyCredentialRegistration
            {
                continuation?.resume(returning: registration)
            } else {
                continuation?.resume(
                    throwing: NSError(
                        domain: "NewUserPasskeyRegistration", code: -1,
                        userInfo: [NSLocalizedDescriptionKey: "Invalid credential type"]))
            }
            continuation = nil
        }

        func authorizationController(
            controller: ASAuthorizationController, didCompleteWithError error: Error
        ) {
            continuation?.resume(throwing: error)
            continuation = nil
        }
    }

    #Preview {
        WebAuthnSignUpView()
            .environmentObject(NativeAuthManager())
    }
#else
    // macOS stub - this view is iOS-only due to AuthenticationServices dependencies
    import SwiftUI

    struct WebAuthnSignUpView: View {
        var body: some View {
            Text("WebAuthn sign-up is not available on macOS")
                .font(.headline)
                .padding()
        }
    }

    #Preview {
        WebAuthnSignUpView()
    }
#endif
