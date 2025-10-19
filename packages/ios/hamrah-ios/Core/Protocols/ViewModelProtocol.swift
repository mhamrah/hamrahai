//
//  ViewModelProtocol.swift
//  hamrah-ios
//
//  Protocol for standardizing view model behavior across the app
//

import Combine
import Foundation

// MARK: - ViewModelProtocol

@MainActor
protocol ViewModelProtocol: ObservableObject {
    var isLoading: Bool { get set }
    var errorMessage: String? { get set }
    var cancellables: Set<AnyCancellable> { get set }

    func handleError(_ error: Error)
    func clearError()
    func setLoading(_ loading: Bool)
}

// MARK: - Default Implementation

extension ViewModelProtocol {
    func handleError(_ error: Error) {
        setLoading(false)
        errorMessage = APIErrorHandler.handle(error)
    }

    func clearError() {
        errorMessage = nil
    }

    func setLoading(_ loading: Bool) {
        isLoading = loading
        if loading {
            clearError()
        }
    }
}

// MARK: - Base ViewModel

@MainActor
class BaseViewModel: ViewModelProtocol {
    @Published var isLoading: Bool = false
    @Published var errorMessage: String?
    var cancellables = Set<AnyCancellable>()

    deinit {
        cancellables.removeAll()
    }
}

// MARK: - API Error Handler

struct APIErrorHandler {
    static func handle(_ error: Error) -> String {
        switch error {
        case let apiError as APIError:
            return handleAPIError(apiError)
        case let urlError as URLError:
            return handleURLError(urlError)
        case let decodingError as DecodingError:
            return handleDecodingError(decodingError)
        default:
            return "An unexpected error occurred: \(error.localizedDescription)"
        }
    }

    private static func handleAPIError(_ error: APIError) -> String {
        return error.errorDescription ?? "An unknown API error occurred."
    }

    private static func handleURLError(_ error: URLError) -> String {
        switch error.code {
        case .notConnectedToInternet:
            return "No internet connection. Please check your network settings."
        case .timedOut:
            return "Request timed out. Please try again."
        case .cannotFindHost:
            return "Cannot connect to server. Please check your connection."
        case .networkConnectionLost:
            return "Network connection lost. Please try again."
        default:
            return "Network error: \(error.localizedDescription)"
        }
    }

    private static func handleDecodingError(_ error: DecodingError) -> String {
        switch error {
        case .dataCorrupted:
            return "Data format error. Please try again."
        case .keyNotFound(let key, _):
            return "Missing data field: \(key.stringValue)"
        case .typeMismatch(let type, _):
            return "Data type error: expected \(type)"
        case .valueNotFound(let type, _):
            return "Missing required value of type \(type)"
        @unknown default:
            return "Data parsing error"
        }
    }
}


// MARK: - Result Extensions

extension Result {
    func handleError<VM: ViewModelProtocol>(in viewModel: VM) {
        if case .failure(let error) = self {
            Task { @MainActor in
                viewModel.handleError(error)
            }
        }
    }
}

// MARK: - Publisher Extensions

extension Publisher {
    func handleLoading<VM: ViewModelProtocol>(in viewModel: VM) -> AnyPublisher<Output, Failure> {
        self
            .handleEvents(
                receiveSubscription: { _ in
                    Task { @MainActor in
                        viewModel.setLoading(true)
                    }
                },
                receiveCompletion: { _ in
                    Task { @MainActor in
                        viewModel.setLoading(false)
                    }
                }
            )
            .receive(on: DispatchQueue.main)
            .eraseToAnyPublisher()
    }

    func handleErrors<VM: ViewModelProtocol>(in viewModel: VM) -> AnyPublisher<Output, Never> {
        self
            .catch { error -> Empty<Output, Never> in
                Task { @MainActor in
                    viewModel.handleError(error)
                }
                return Empty()
            }
            .eraseToAnyPublisher()
    }
}
