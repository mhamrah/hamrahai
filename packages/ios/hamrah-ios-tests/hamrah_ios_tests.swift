//
//  hamrah_ios_tests.swift
//  hamrah-ios-tests
//
//  Created by Mike Hamrah on 10/12/25.
//

import Foundation
import SwiftData
import Testing
@testable import hamrah_ios

#if os(iOS)
    import DeviceCheck
#endif

@MainActor
struct hamrah_ios_tests {

    final class MockLinkAPI: LinkAPI {
        var postCallCount = 0
        var getCallCount = 0

        func postLink(payload: OutboundLinkPayload) async throws -> PostLinkResponse {
            postCallCount += 1
            return PostLinkResponse(serverId: payload.clientId, canonicalUrl: payload.url)
        }

        func getLinks(since: String, limit: Int) async throws -> DeltaResponse {
            getCallCount += 1
            return DeltaResponse(links: [], nextCursor: nil)
        }
    }

    @Test func example() async throws {
        // Write your test here and use APIs like `#expect(...)` to check expected conditions.
    }

    @Test func deviceCheckInvalidInputIsRecoverableForAttestationRetry() {
        #if os(iOS)
            let error = NSError(
                domain: DCErrorDomain,
                code: DCError.Code.invalidInput.rawValue
            )

            #expect(HamrahAPIClient.isRecoverableAttestationError(error))
        #endif
    }

    @Test func appAttestationKeyGenerationFailureIsRecoverable() {
        #if os(iOS)
            #expect(
                HamrahAPIClient.isRecoverableAttestationError(
                    AttestationError.keyGenerationFailed("Key invalidated")
                )
            )
        #endif
    }

    @Test func unrelatedErrorIsNotRecoverableForAttestationRetry() {
        let error = NSError(domain: NSURLErrorDomain, code: NSURLErrorNotConnectedToInternet)

        #expect(!HamrahAPIClient.isRecoverableAttestationError(error))
    }

    @MainActor
    @Test func nativeAuthPayloadUsesOnlyCanonicalIdentityTokenField() throws {
        let payload = NativeAuthManager.backendAuthPayload(
            provider: "apple",
            credential: "apple.jwt",
            platform: "ios",
            additionalData: [
                "email": "person@example.com",
                "name": "Person Example",
                "provider_id": "apple-user-id",
            ]
        )

        #expect(payload["provider"] == "apple")
        #expect(payload["id_token"] == "apple.jwt")
        #expect(payload["platform"] == "ios")
        #expect(payload["auth_method"] == "apple")
        #expect(payload["email"] == "person@example.com")
        #expect(payload["credential"] == nil)

        let encoded = try JSONEncoder().encode(payload)
        let decoded = try #require(JSONSerialization.jsonObject(with: encoded) as? [String: String])

        #expect(decoded["id_token"] == "apple.jwt")
        #expect(decoded["credential"] == nil)
    }

    @MainActor
    @Test func nativeAuthPayloadDoesNotAllowMetadataToOverrideReservedFields() {
        let payload = NativeAuthManager.backendAuthPayload(
            provider: "google",
            credential: "google.jwt",
            platform: "ios",
            additionalData: [
                "provider": "apple",
                "id_token": "attacker.jwt",
                "platform": "web",
                "auth_method": "password",
                "email": "person@example.com",
            ]
        )

        #expect(payload["provider"] == "google")
        #expect(payload["id_token"] == "google.jwt")
        #expect(payload["platform"] == "ios")
        #expect(payload["auth_method"] == "google")
        #expect(payload["email"] == "person@example.com")
        #expect(payload["credential"] == nil)
    }

    @Test func webAuthnBeginResponseDecodesSnakeCaseSuccessPayload() throws {
        let jsonData = """
            {
                "success": true,
                "options": {
                    "challenge": "test-challenge-base64",
                    "challenge_id": "test-challenge-id",
                    "rp_id": "hamrah.app",
                    "timeout": 60000,
                    "allow_credentials": []
                },
                "challenge_id": "test-challenge-id"
            }
            """.data(using: .utf8)!

        let response = try JSONDecoder().decode(
            NativeAuthManager.WebAuthnBeginResponse.self,
            from: jsonData
        )

        #expect(response.success)
        #expect(response.options?.challengeId == "test-challenge-id")
        #expect(response.challengeId == "test-challenge-id")
    }

    @Test func webAuthnBeginResponseDecodesBackendFailurePayload() throws {
        let jsonData = """
            {
                "success": false,
                "error": "User not found"
            }
            """.data(using: .utf8)!

        let response = try JSONDecoder().decode(
            NativeAuthManager.WebAuthnBeginResponse.self,
            from: jsonData
        )

        #expect(!response.success)
        #expect(response.options == nil)
        #expect(response.challengeId == nil)
        #expect(response.error == "User not found")
    }

    @Test func deviceCheckUnknownSystemFailureUsesRecoveryThenFallbackWhenAuthenticated() {
        #if os(iOS)
            let error = NSError(
                domain: DCErrorDomain,
                code: DCError.Code.unknownSystemFailure.rawValue
            )

            #expect(HamrahAPIClient.isRecoverableAttestationError(error))
            #expect(
                HamrahAPIClient.attestationFailureStrategy(
                    for: error,
                    accessToken: "access-token"
                ) == .recoverThenFallback
            )
        #endif
    }

    @Test func deviceCheckUnknownSystemFailureFallsBackForUnauthenticatedAuthBootstrap() {
        #if os(iOS)
            let error = NSError(
                domain: DCErrorDomain,
                code: DCError.Code.unknownSystemFailure.rawValue
            )

            #expect(
                HamrahAPIClient.attestationFailureStrategy(
                    for: error,
                    accessToken: nil
                ) == .fallback
            )
        #endif
    }

    @Test func fallbackAttestationHeadersMatchServerBypassContract() {
        let headers = HamrahAPIClient.fallbackAttestationHeaders(
            bundleIdentifier: "app.hamrah.ios",
            appVersion: "1.2.3"
        )

        #expect(headers["X-App-Attestation-Mode"] == "none")
        #expect(headers["X-iOS-Bundle-ID"] == "app.hamrah.ios")
        #expect(headers["X-iOS-App-Version"] == "1.2.3")
    }

    @Test func syncDomainApiDoesNotExposeAccessTokenArguments() async throws {
        let config = ModelConfiguration(isStoredInMemoryOnly: true)
        let container = try ModelContainer(
            for: LinkEntity.self, TagEntity.self, SyncCursor.self, UserPrefs.self,
            configurations: config
        )
        let context = container.mainContext
        let url = URL(string: "https://example.com/refresh")!
        let link = LinkEntity(
            originalUrl: url,
            canonicalUrl: url,
            sharedAt: Date(),
            status: "queued",
            updatedAt: Date(),
            createdAt: Date()
        )
        context.insert(link)
        try context.save()

        let api = MockLinkAPI()
        let engine = SyncEngine(
            api: api,
            modelContainer: container
        )

        await engine._testRunSyncNow(reason: "test_domain_api_without_token_surface")

        #expect(api.postCallCount == 1)
        #expect(api.getCallCount == 1)
    }

}
