//
//  hamrah_ios_tests.swift
//  hamrah-ios-tests
//
//  Created by Mike Hamrah on 10/12/25.
//

import Foundation
import Testing
@testable import hamrah_ios

#if os(iOS)
    import DeviceCheck
#endif

struct hamrah_ios_tests {

    @Test func example() async throws {
        // Write your test here and use APIs like `#expect(...)` to check expected conditions.
    }

    @Test func deviceCheckInvalidInputIsRecoverableForAttestationRetry() {
        #if os(iOS)
            let error = NSError(
                domain: DCErrorDomain,
                code: DCError.Code.invalidInput.rawValue
            )

            #expect(SecureAPIService.isRecoverableAttestationError(error))
        #endif
    }

    @Test func appAttestationKeyGenerationFailureIsRecoverable() {
        #if os(iOS)
            #expect(
                SecureAPIService.isRecoverableAttestationError(
                    AttestationError.keyGenerationFailed("Key invalidated")
                )
            )
        #endif
    }

    @Test func unrelatedErrorIsNotRecoverableForAttestationRetry() {
        let error = NSError(domain: NSURLErrorDomain, code: NSURLErrorNotConnectedToInternet)

        #expect(!SecureAPIService.isRecoverableAttestationError(error))
    }

}
