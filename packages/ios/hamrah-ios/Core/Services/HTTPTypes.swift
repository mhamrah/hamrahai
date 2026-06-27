import Foundation

enum HTTPMethod: String {
    case GET = "GET"
    case HEAD = "HEAD"
    case POST = "POST"
    case PUT = "PUT"
    case DELETE = "DELETE"
    case PATCH = "PATCH"
}

enum APIError: LocalizedError {
    case invalidResponse
    case unauthorized
    case serverError(Int, String)
    case attestationFailed(String)
    case simulatorNotSupported

    var errorDescription: String? {
        switch self {
        case .invalidResponse:
            return "Invalid response from server"
        case .unauthorized:
            return "Authentication required. Please sign in again."
        case .serverError(let code, let message):
            return "Server error (\(code)): \(message)"
        case .attestationFailed(let details):
            #if targetEnvironment(simulator)
                return
                    "App verification not supported on simulator. Please test on a physical device."
            #else
                return "App verification failed: \(details)"
            #endif
        case .simulatorNotSupported:
            return
                "This feature requires a physical iOS device and is not supported on the simulator."
        }
    }
}
