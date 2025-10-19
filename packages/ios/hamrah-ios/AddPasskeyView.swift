//
//  AddPasskeyView.swift
//  hamrahIOS
//
//  Add Passkey view for iOS app
//

import AuthenticationServices
import SwiftUI

struct AddPasskeyView: View {
    @Environment(\.presentationMode) var presentationMode
    @EnvironmentObject var authManager: NativeAuthManager
    @State private var isLoading = false
    @State private var errorMessage: String?
    @State private var showErrorAlert = false
    let onPasskeyAdded: () -> Void

    var body: some View {
        NavigationView {
            VStack(spacing: 24) {
                Spacer()

                // Icon
                Image(systemName: "key.fill")
                    .font(.system(size: 60))
                    .foregroundColor(.blue)

                // Title and Description
                VStack(spacing: 16) {
                    Text("Add Passkey")
                        .font(.title)
                        .fontWeight(.bold)

                    Text(
                        "Passkeys provide secure, passwordless authentication using your device's biometrics or PIN."
                    )
                    .font(.body)
                    .multilineTextAlignment(.center)
                    .foregroundColor(.secondary)
                    .padding(.horizontal)
                }

                Spacer()

                // Add Passkey Button
                Button(action: addPasskey) {
                    HStack {
                        if isLoading {
                            ProgressView()
                                .scaleEffect(0.8)
                                .tint(.white)
                        } else {
                            Image(systemName: "plus.circle.fill")
                                .font(.title3)
                        }
                        Text(isLoading ? "Creating..." : "Add Passkey")
                            .fontWeight(.semibold)
                    }
                    .foregroundColor(.white)
                    .frame(maxWidth: .infinity)
                    .padding()
                    .background(Color.blue)
                    .cornerRadius(12)
                }
                .disabled(isLoading)

                // Cancel Button
                Button("Cancel") {
                    presentationMode.wrappedValue.dismiss()
                }
                .foregroundColor(.secondary)
                .disabled(isLoading)

                Spacer()
            }
            .padding()
            #if os(iOS)
                .navigationTitle("Add Passkey")
                .navigationBarTitleDisplayMode(.inline)
                .navigationBarItems(
                    trailing: Button("Done") {
                        presentationMode.wrappedValue.dismiss()
                    }.disabled(isLoading)
                )
            #elseif os(macOS)
                // macOS: still show a title, and give the sheet/content a sensible default size
                .navigationTitle("Add Passkey")
                .frame(minWidth: 420, minHeight: 520)
            #endif
            .alert("Error", isPresented: $showErrorAlert) {
                Button("OK") {
                    errorMessage = nil
                    showErrorAlert = false
                }
            } message: {
                Text(errorMessage ?? "")
            }
        }
    }

    private func addPasskey() {
        // Debug authentication state
        print("🔍 Authentication Debug:")
        print("  Current User: \(authManager.currentUser?.email ?? "nil")")
        print("  Access Token: \(authManager.accessToken != nil ? "present" : "nil")")
        print("  Is Authenticated: \(authManager.isAuthenticated)")

        guard let user = authManager.currentUser else {
            errorMessage = "No user found. Please sign in again."
            showErrorAlert = true
            return
        }

        guard let accessToken = authManager.accessToken else {
            errorMessage = "No access token found. Please sign in again."
            showErrorAlert = true
            return
        }

        isLoading = true
        errorMessage = nil

        Task {
            do {
                try await registerPasskey(email: user.email, accessToken: accessToken)
                await MainActor.run {
                    self.isLoading = false
                    self.onPasskeyAdded()
                    self.presentationMode.wrappedValue.dismiss()
                }
            } catch {
                await MainActor.run {
                    self.errorMessage = "Failed to add passkey: \(error.localizedDescription)"
                    self.isLoading = false
                    self.showErrorAlert = true
                }
            }
        }
    }

    private func registerPasskey(email: String, accessToken: String) async throws {
        // Step 1: Begin WebAuthn registration
        let beginOptions = try await beginWebAuthnRegistration(
            email: email, accessToken: accessToken)

        guard let options = beginOptions.options else {
            throw NSError(
                domain: "WebAuthn", code: -1,
                userInfo: [NSLocalizedDescriptionKey: "No registration options received"])
        }

        let challengeId = beginOptions.challengeId

        // Step 2: Perform platform registration
        let attestation = try await performPlatformRegistration(options: options)

        // Step 3: Verify registration with backend
        try await completeWebAuthnRegistration(
            attestation: attestation, challengeId: challengeId, email: email,
            accessToken: accessToken)
    }

    private func beginWebAuthnRegistration(email: String, accessToken: String) async throws
        -> WebAuthnBeginRegistrationResponse
    {
        let body = [
            "user_id": authManager.currentUser?.id ?? "",
            "email": email,
            "display_name": authManager.currentUser?.name ?? email,
            "label": "iOS Device",
        ]

        return try await SecureAPIService.shared.post(
            endpoint: "/api/webauthn/register/begin",
            body: body,
            accessToken: accessToken,
            responseType: WebAuthnBeginRegistrationResponse.self,
            customBaseURL: APIConfiguration.shared.baseURL
        )
    }

    private func performPlatformRegistration(options: PublicKeyCredentialCreationOptions)
        async throws -> ASAuthorizationPlatformPublicKeyCredentialRegistration
    {
        let challenge = Data(base64URLEncoded: options.challenge) ?? Data()
        let userID = Data(base64URLEncoded: options.user.id) ?? Data()

        let request = ASAuthorizationPlatformPublicKeyCredentialProvider(
            relyingPartyIdentifier: options.rp.id
        )
        .createCredentialRegistrationRequest(
            challenge: challenge, name: options.user.name, userID: userID)

        return try await withCheckedThrowingContinuation { continuation in
            let controller = ASAuthorizationController(authorizationRequests: [request])

            // Store continuation for delegate callback
            PasskeyRegistrationDelegate.shared.setContinuation(continuation)

            controller.delegate = PasskeyRegistrationDelegate.shared
            controller.presentationContextProvider = authManager
            controller.performRequests()
        }
    }

    private func completeWebAuthnRegistration(
        attestation: ASAuthorizationPlatformPublicKeyCredentialRegistration, challengeId: String,
        email: String, accessToken: String
    ) async throws {
        // Create the response object matching SimpleWebAuthn's RegistrationResponseJSON format
        let registrationResponseData =
            [
                "id": attestation.credentialID.base64URLEncodedString(),
                "rawId": attestation.credentialID.base64URLEncodedString(),
                "type": "public-key",
                "response": [
                    "attestationObject": attestation.rawAttestationObject?.base64URLEncodedString()
                        ?? "",
                    "clientDataJSON": attestation.rawClientDataJSON.base64URLEncodedString(),
                ],
            ] as [String: Any]

        let body =
            [
                "response": registrationResponseData,
                "challenge_id": challengeId,
                // Optional friendly label for server-side storage
                "label": "iOS Device",
            ] as [String: Any]

        _ = try await SecureAPIService.shared.post(
            endpoint: "/api/webauthn/register/verify",
            body: body,
            accessToken: accessToken,
            responseType: APIResponse.self,
            customBaseURL: APIConfiguration.shared.baseURL
        )
    }
}

// MARK: - Data Models for Registration

struct WebAuthnBeginRegistrationResponse: Codable {
    let success: Bool
    let options: PublicKeyCredentialCreationOptions?
    let challengeId: String
    let error: String?

    enum CodingKeys: String, CodingKey {
        case success
        case options
        case challengeId = "challenge_id"
        case error
    }
}

struct PublicKeyCredentialCreationOptions: Codable {
    let challenge: String
    let rp: RelyingParty
    let user: UserInfo
    let pubKeyCredParams: [PubKeyCredParam]
    let timeout: Int?
    let excludeCredentials: [PublicKeyCredentialDescriptorForCreation]?
    let authenticatorSelection: AuthenticatorSelection?
    let challengeId: String
}

struct RelyingParty: Codable {
    let id: String
    let name: String
}

struct UserInfo: Codable {
    let id: String
    let name: String
    let displayName: String
}

struct PubKeyCredParam: Codable {
    let type: String
    let alg: Int
}

struct AuthenticatorSelection: Codable {
    let authenticatorAttachment: String?
    let userVerification: String?
    let requireResidentKey: Bool?
}

struct PublicKeyCredentialDescriptorForCreation: Codable {
    let type: String
    let id: String
    let transports: [String]?
}

// MARK: - Base64URL helpers
extension Data {
    init?(base64URLEncoded string: String) {
        var base64 =
            string
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        let padding = 4 - (base64.count % 4)
        if padding < 4 {
            base64.append(String(repeating: "=", count: padding))
        }
        guard let data = Data(base64Encoded: base64) else { return nil }
        self = data
    }

    func base64URLEncodedString() -> String {
        return self.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }
}

// MARK: - Passkey Registration Delegate

class PasskeyRegistrationDelegate: NSObject, ASAuthorizationControllerDelegate {
    static let shared = PasskeyRegistrationDelegate()

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
                    domain: "PasskeyRegistration", code: -1,
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
    AddPasskeyView(onPasskeyAdded: {})
        .environmentObject(NativeAuthManager())
}
