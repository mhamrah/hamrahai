import Foundation

/// Central configuration for API endpoints.
///
/// In production the app talks to `https://api.hamrah.app`.
/// During development (simulator or device) the API is served by a local
/// server at `http://localhost:8080`.
/// The configuration also supports a single custom URL that can be
/// persisted in `UserDefaults`.  If a custom URL is set, HTTPS is
/// enforced in production and HTTP is used in the simulator to avoid
/// TLS handshake failures.
///
/// Only a single endpoint is exposed: `baseURL`.  Any previous web‑app
/// URL logic has been removed for clarity.
@Observable
class APIConfiguration {
    /// The three modes the app can run in.
    enum Environment: String, CaseIterable {
        case production = "Production"
        case development = "Development"
        case custom = "Custom"

        /// Base URL for the current environment.
        var baseURL: String {
            switch self {
            case .production:
                return "https://api.hamrah.app"
            case .development:
                // Use HTTP locally to avoid TLS mismatches.
                return "http://localhost:8080"
            case .custom:
                return ""
            }
        }
    }

    static let shared = APIConfiguration()

    // MARK: - UserDefaults keys
    private let environmentKey = "APIEnvironment"
    private let customApiBaseURLKey = "CustomAPIBaseURL"
    private let legacyCustomBaseURLKey = "CustomBaseURL"
    private let simulatorLocalhostKey = "SimulatorLocalhostEnabled"

    // MARK: - Public properties
    var currentEnvironment: Environment {
        didSet {
            UserDefaults.standard.set(currentEnvironment.rawValue, forKey: environmentKey)
        }
    }

    var customApiBaseURL: String {
        didSet {
            UserDefaults.standard.set(customApiBaseURL, forKey: customApiBaseURLKey)
        }
    }

    /// Legacy support: a single custom URL that previously represented both
    /// the API and the web app.  It now maps only to the API endpoint.
    var customBaseURL: String {
        get { customApiBaseURL }
        set {
            customApiBaseURL = newValue
            UserDefaults.standard.set(newValue, forKey: legacyCustomBaseURLKey)
        }
    }

    /// Toggle: when running on Simulator, force localhost:8080 unless disabled via this switch.
    var simulatorLocalhostEnabled: Bool {
        didSet {
            UserDefaults.standard.set(simulatorLocalhostEnabled, forKey: simulatorLocalhostKey)
        }
    }

    // MARK: - Initializer
    init() {
        // Load environment or default to production.
        if let saved = UserDefaults.standard.string(forKey: environmentKey),
            let env = Environment(rawValue: saved)
        {
            self.currentEnvironment = env
        } else {
            self.currentEnvironment = .production
        }

        // Load custom base URL with legacy migration support.
        let defaults = UserDefaults.standard
        let legacy = defaults.string(forKey: legacyCustomBaseURLKey) ?? ""
        self.customApiBaseURL = defaults.string(forKey: customApiBaseURLKey) ?? legacy

        // Initialize simulator localhost toggle
        #if targetEnvironment(simulator)
            if UserDefaults.standard.object(forKey: simulatorLocalhostKey) != nil {
                self.simulatorLocalhostEnabled = UserDefaults.standard.bool(
                    forKey: simulatorLocalhostKey)
            } else {
                self.simulatorLocalhostEnabled = true
            }
        #else
            self.simulatorLocalhostEnabled = false
        #endif
    }

    // MARK: - Convenience for tests
    static func testInstance() -> APIConfiguration {
        let config = APIConfiguration()
        config.currentEnvironment = .production
        return config
    }

    // MARK: - Computed URLs
    var baseURL: String {
        switch currentEnvironment {
        case .production, .development:
            #if targetEnvironment(simulator)
                if simulatorLocalhostEnabled && customApiBaseURL.isEmpty {
                    return "http://localhost:8080"
                } else {
                    return currentEnvironment.baseURL
                }
            #else
                return currentEnvironment.baseURL
            #endif
        case .custom:
            let url = customApiBaseURL
            if url.isEmpty { return Environment.production.baseURL }

            #if targetEnvironment(simulator)
                // In simulator, downgrade HTTPS to HTTP to avoid TLS handshake issues.
                if url.hasPrefix("https://") {
                    return url.replacingOccurrences(of: "https://", with: "http://")
                }
                return url
            #else
                // In production, enforce HTTPS.
                if url.hasPrefix("http://") {
                    return url.replacingOccurrences(of: "http://", with: "https://")
                } else if !url.hasPrefix("https://") {
                    return "https://\(url)"
                }
                return url
            #endif
        }
    }

    // MARK: - URL configuration helpers
    func setCustomURL(_ url: String) {
        customApiBaseURL = url
        currentEnvironment = .custom
    }

    func setCustomApiURL(_ url: String) {
        customApiBaseURL = url
        currentEnvironment = .custom
    }

    func reset() {
        currentEnvironment = .production
        customApiBaseURL = ""
    }
}
