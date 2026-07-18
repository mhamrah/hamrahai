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
        var capturedPostPayloads: [OutboundLinkPayload] = []
        var nextPostResult: Result<PostLinkResponse, Error> = .success(
            PostLinkResponse(serverId: "srv-1", canonicalUrl: "https://example.com/canonical")
        )
        var nextDeltaResult: Result<DeltaResponse, Error> = .success(
            DeltaResponse(links: [], nextCursor: "cursor-next")
        )

        func postLink(payload: OutboundLinkPayload) async throws -> PostLinkResponse {
            postCallCount += 1
            capturedPostPayloads.append(payload)
            return try nextPostResult.get()
        }

        func getLinks(since: String, limit: Int) async throws -> DeltaResponse {
            getCallCount += 1
            return try nextDeltaResult.get()
        }

        func updateLink(serverId: String, status: String) async throws -> ServerLink {
            ServerLink(
                serverId: serverId,
                originalUrl: "https://example.com",
                canonicalUrl: "https://example.com",
                title: nil,
                snippet: nil,
                summaryShort: nil,
                summaryLong: nil,
                lang: nil,
                tags: [],
                saveCount: 1,
                status: status,
                sharedAt: Date(),
                createdAt: Date()
            )
        }

        func deleteLink(serverId: String) async throws {}
    }

    enum SyncTestError: Error {
        case network
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

    @Test func appAttestationChallengeResponseDecodesVerifierChallengeBytes() throws {
        #if os(iOS)
            let verifierChallenge = "EJbEGFFbB7vxU1+e5bZ6TOsB4b6keGHe3Ih0pQlK7FI="
            let responseChallenge = Data(verifierChallenge.utf8).base64EncodedString()
            let jsonData = """
                {
                    "success": true,
                    "challenge": "\(responseChallenge)",
                    "challenge_id": "challenge-id",
                    "error": null
                }
                """.data(using: .utf8)!

            let response = try JSONDecoder().decode(
                AttestationChallengeResponse.self,
                from: jsonData
            )
            let challengeData = try #require(
                response.challenge.flatMap { Data(base64Encoded: $0) }
            )

            #expect(response.success)
            #expect(response.challengeId == "challenge-id")
            #expect(challengeData == Data(verifierChallenge.utf8))
        #endif
    }

    @Test func appAttestationRequestChallengeDataIsSignedJsonWithChallenge() throws {
        let url = try #require(URL(string: "https://api.hamrah.app/v1/user/prefs"))
        let body = #"{"default_model":"@cf/zai-org/glm-4.7-flash"}"#.data(using: .utf8)!
        let clientData = try HamrahAPIClient.requestChallengeData(
            url: url,
            method: .PUT,
            body: body,
            issuedAt: Date(timeIntervalSince1970: 1_782_700_000),
            challenge: "request-nonce"
        )
        let decoded = try JSONDecoder().decode(AppAttestRequestClientData.self, from: clientData)

        #expect(decoded.challenge == "request-nonce")
        #expect(decoded.method == "PUT")
        #expect(decoded.url == "https://api.hamrah.app/v1/user/prefs")
        #expect(
            decoded.bodySha256
                == "1747b5a7871b52686eb992730413af1f62df1d0ca9a814ca6a8b4737c20a1a7c"
        )
        #expect(decoded.issuedAt == 1_782_700_000)
    }

    @Test func unauthorizedAppAttestServerErrorMapsToAttestationError() throws {
        let data = #"{"success":false,"error":"App Attest assertion verification failed"}"#
            .data(using: .utf8)!

        #expect(
            HamrahAPIClient.unauthorizedResponseError(data: data)
                == .attestation("App Attest assertion verification failed")
        )
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

    @MainActor
    @Test func nativeAuthPayloadMarksProviderLinkingRequests() {
        let payload = NativeAuthManager.backendAuthPayload(
            provider: "google",
            credential: "google.jwt",
            platform: "ios",
            linkProvider: true
        )

        #expect(payload["link_provider"] == "true")
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

    @Test func passkeyPathComponentsCannotEscapeTheirRouteSegment() {
        #expect(PasskeyAPI.pathComponent("user/with/slash") == "user%2Fwith%2Fslash")
        #expect(PasskeyAPI.pathComponent("credential?id=1") == "credential%3Fid%3D1")
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

    @Test func outboundSyncPostsQueuedLinksAndUpdatesStatus() async throws {
        let container = try makeInMemoryLinkContainer()
        let context = container.mainContext
        let original = URL(string: "https://example.com/path")!
        let link = LinkEntity(
            originalUrl: original,
            canonicalUrl: original,
            sharedAt: Date(),
            status: "queued",
            updatedAt: Date(),
            createdAt: Date()
        )
        context.insert(link)
        try context.save()

        let api = MockLinkAPI()
        api.nextPostResult = .success(
            PostLinkResponse(serverId: "server-123", canonicalUrl: "https://example.com/canonical")
        )
        let engine = SyncEngine(api: api, modelContainer: container)

        await engine._testRunSyncNow(reason: "test_outbound_success")

        let saved = try #require(fetchLinks(context).first)
        #expect(saved.status == "synced")
        #expect(saved.serverId == "server-123")
        #expect(saved.canonicalUrl.absoluteString == "https://example.com/canonical")
        #expect(api.capturedPostPayloads.count == 1)
        #expect(api.capturedPostPayloads.first?.url == "https://example.com/path")
    }

    @Test func outboundSyncFailureLeavesLinkQueuedWithError() async throws {
        let container = try makeInMemoryLinkContainer()
        let context = container.mainContext
        let url = URL(string: "https://example.com/failure")!
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
        api.nextPostResult = .failure(SyncTestError.network)
        let engine = SyncEngine(api: api, modelContainer: container)

        await engine._testRunSyncNow(reason: "test_outbound_failure")

        let saved = try #require(fetchLinks(context).first)
        #expect(saved.status == "queued")
        #expect(saved.attempts == 1)
        #expect(saved.lastError != nil)
    }

    @Test func inboundSyncMergesServerLinksAndUpdatesCursor() async throws {
        let container = try makeInMemoryLinkContainer()
        let context = container.mainContext
        let now = Date()
        let serverLink = ServerLink(
            serverId: "server-789",
            originalUrl: "https://example.com/original",
            canonicalUrl: "https://example.com/canonical",
            title: "Server Title",
            snippet: "Snippet",
            summaryShort: "Short",
            summaryLong: "Long",
            lang: "en",
            tags: ["swift", "sync"],
            saveCount: 3,
            status: "synced",
            sharedAt: now,
            createdAt: now.addingTimeInterval(-3600)
        )

        let api = MockLinkAPI()
        api.nextDeltaResult = .success(
            DeltaResponse(links: [serverLink], nextCursor: "cursor-2")
        )
        let engine = SyncEngine(api: api, modelContainer: container)

        await engine._testRunSyncNow(reason: "test_inbound")

        let saved = try #require(fetchLinks(context).first)
        #expect(saved.serverId == "server-789")
        #expect(saved.title == "Server Title")
        #expect(saved.snippet == "Snippet")
        #expect(saved.summaryShort == "Short")
        #expect(saved.summaryLong == "Long")
        #expect(saved.lang == "en")
        #expect(saved.saveCount == 3)
        #expect(saved.status == "synced")
        #expect(saved.canonicalUrl.absoluteString == "https://example.com/canonical")
        #expect(saved.tags.map(\.name).sorted() == ["swift", "sync"])

        let cursor = try #require((try? context.fetch(FetchDescriptor<SyncCursor>()))?.first)
        #expect(cursor.lastUpdatedCursor == "cursor-2")
        #expect(cursor.lastFullSyncAt != nil)
    }

    private func makeInMemoryLinkContainer() throws -> ModelContainer {
        let config = ModelConfiguration(isStoredInMemoryOnly: true)
        return try ModelContainer(
            for: LinkEntity.self, TagEntity.self, SyncCursor.self, UserPrefs.self,
            configurations: config
        )
    }

    private func fetchLinks(_ context: ModelContext) -> [LinkEntity] {
        (try? context.fetch(FetchDescriptor<LinkEntity>())) ?? []
    }

}
