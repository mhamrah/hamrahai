# Hamrah iOS App - Agent Instructions

## Overview
Hamrah iOS is a native Swift application that provides secure authentication and user management, serving as a client to the hamrah-api backend. The app follows an **offline-first architecture** with comprehensive cross-platform support for iOS and macOS, utilizing modern Swift development patterns and enterprise-grade security.

## 🎯 Core Principles for Agents

### 1. Performance & Architecture Requirements
- **CRITICAL**: All code must be performant and well-modularized for both iOS and macOS
- **CRITICAL**: Maintain 100% unit test coverage for all new functionality
- **CRITICAL**: Follow offline-first approach - app must function without internet connectivity
- **CRITICAL**: Use hamrah-api for ALL data persistence - no local data storage except for caching

### 2. Code Quality Standards
- **Modular Architecture**: Follow the established feature-based architecture
- **Cross-Platform**: Use platform-specific components from `Shared/Components/`
- **SwiftUI Only**: All UI must use SwiftUI - UIKit usage is prohibited except for necessary platform abstractions in `PlatformBridge.swift`, authentication contexts, and App Attestation
- **Theme Consistency**: Apply `Theme.swift` system for all UI elements
- **Performance First**: Optimize queries, implement pagination, use efficient data structures
- **Type Safety**: Leverage Swift's type system for compile-time error prevention

## 🏗️ Architecture Overview

### Current Structure (MANDATORY to follow)
```
hamrah-ios/
├── Core/
│   ├── Data/QueryDescriptors.swift      # Optimized SwiftData queries
│   ├── Managers/                        # Business logic coordinators
│   ├── Models/                          # SwiftData models
│   ├── Protocols/ViewModelProtocol.swift # Standardized VM interface
│   └── Services/                        # API, Keychain, Security services
├── Features/
│   ├── Authentication/                  # Login, biometric, OAuth flows
│   ├── Inbox/                          # Link management UI
│   ├── Settings/                       # App configuration
│   └── ShareExtension/                 # Share handling
├── Shared/
│   ├── Components/                     # Cross-platform UI components
│   ├── Theme/Theme.swift              # Design system
│   └── Utilities/                     # Helper functions
└── Platform/
    ├── iOS/                           # iOS-specific implementations
    └── macOS/                         # macOS-specific implementations
```

### Core Features
- **Secure Authentication**: Apple Sign-In, Google Sign-In, WebAuthn passkeys
- **Link Management**: Save, sync, and manage URLs with rich metadata
- **Offline-First**: Full functionality without internet, sync when available
- **Cross-Platform**: Native iOS and macOS experience

## 🛡️ Security Architecture (NON-NEGOTIABLE)

### Authentication Flow
1. **OAuth Login**: Apple/Google OAuth handled natively
2. **Token Exchange**: OAuth tokens exchanged for hamrah-api access tokens
3. **App Attestation**: iOS DeviceCheck validates app authenticity
4. **Secure Storage**: Tokens stored in Keychain with biometric protection
5. **API Communication**: All requests include attestation headers

### Security Requirements
- **NEVER store sensitive data in UserDefaults** - Keychain only
- **ALWAYS include App Attestation headers** for API calls
- **IMPLEMENT biometric authentication** for sensitive operations
- **USE certificate pinning** for API communications
- **VALIDATE all API responses** and handle errors gracefully

## 📊 Data Strategy (CRITICAL)

### Offline-First Approach
- **Local SwiftData Store**: Cache for offline functionality using App Group
- **Sync Engine**: Bidirectional sync with hamrah-api when online
- **Conflict Resolution**: Server-side resolution for data conflicts
- **Background Sync**: Automatic sync using BGTaskScheduler

### API Integration
- **Endpoint**: `https://api.hamrah.app/api/`
- **Protocol**: JSON over HTTPS with App Attestation
- **All Persistence**: Use hamrah-api for authoritative data storage
- **Local Storage**: Only for caching and offline functionality
- **Request Boundary**: Feature/domain code must call domain APIs (`LinkAPI`, `UserPrefsAPI`, `PasskeyAPI`, `ModelCatalogAPI`, `AuthAPI`-style wrappers), not `URLSession`, `SecureAPIService`, or token-taking helpers.
- **Transport**: `HamrahAPIClient` owns URL construction, snake_case JSON encoding/decoding, `Authorization: Bearer`, App Attestation headers, trace headers, 401 retry, and typed errors.
- **Session**: `SessionManager` owns Keychain token reads/writes, refresh rotation, expiry checks, and invalid-session clearing. `KeychainManager` is storage-only.
- **Policies and Errors**: Use `auth: .none | .optional | .required` at the domain boundary. Standard categories are `unauthorized`, `session_expired`, `server`, `network`, `decoding`, and `attestation`.

## 🎨 UI/UX Standards (MANDATORY)

### SwiftUI-Only Development
**CRITICAL**: This project uses SwiftUI exclusively for all user interface code.

**✅ ALLOWED UIKit Usage (Very Limited)**:
- `PlatformBridge.swift` - Platform abstraction utilities
- `NativeAuthManager.swift` - Authentication presentation contexts
- `AppAttestationManager.swift` - iOS App Attestation APIs
- `WebView` wrappers - For displaying web content (UIViewRepresentable/NSViewRepresentable)

**❌ PROHIBITED UIKit Usage**:
- `UIAlertController` - Use SwiftUI alerts instead
- `UIActivityViewController` - Use SwiftUI `ShareLink` instead
- `UIApplication.shared.open()` - Use SwiftUI `@Environment(\.openURL)` or `PlatformBridge.openURL()`
- `UIPasteboard` - Use `PlatformBridge.copyToClipboard()` instead
- Direct `UIColor` references - Use SwiftUI `Color(.systemBackground)` format

### Use Established Components
```swift
// ✅ CORRECT - Use platform components
PlatformButton("Save", systemImage: "checkmark", style: .primary) {
    viewModel.save()
}

// ❌ WRONG - Platform-specific code
#if os(iOS)
Button("Save") { }.buttonStyle(.borderedProminent)
#else
Button("Save") { }.buttonStyle(.bordered)
#endif
```

### Theme System Usage
```swift
// ✅ CORRECT - Use theme system
Text("Title")
    .font(Theme.Typography.cardTitle)
    .foregroundColor(Theme.Colors.primaryText)
    .themedCard()

// ❌ WRONG - Magic numbers and inconsistent styling
Text("Title")
    .font(.system(size: 18, weight: .medium))
    .foregroundColor(.primary)
    .padding(16)
```

### Performance Requirements
- **Pagination**: Limit initial queries to 50 items maximum
- **Debounced Search**: 300ms minimum delay for search operations
- **Lazy Loading**: Load content only when visible
- **Memory Management**: Proper cleanup in view models and cancellables

## 🧪 Testing Requirements (CRITICAL)

### Unit Test Coverage
- **100% coverage** for all new ViewModels
- **100% coverage** for all Core services and managers
- **Test offline scenarios** and sync conflicts
- **Mock all API dependencies** for reliable testing
- Before changing a user workflow, inspect the matching web implementation and shared API contract. Native-only completion is valid only when the sibling client is verified unaffected and that evidence is recorded in the PR.
- Test changed controls through their state transition and verify the domain API receives the selected snake_case request values. A DTO or computed-property test alone is not sufficient when the defect crossed the view/API boundary.
- Verify both iOS and macOS compilation for shared Settings or feature code, plus loading, retry, disabled-state, Dynamic Type, and accessibility behavior.

### Test Structure
```swift
// ✅ REQUIRED pattern for all tests
class InboxViewModelTests: XCTestCase {
    var viewModel: InboxViewModel!
    var mockAPI: MockLinkAPI!
    var mockModelContext: ModelContext!

    override func setUp() {
        super.setUp()
        mockAPI = MockLinkAPI()
        mockModelContext = createInMemoryContext()
        viewModel = InboxViewModel(api: mockAPI, modelContext: mockModelContext)
    }

    func testLoadLinks_Success() async {
        // Given, When, Then pattern
    }
}
```

## 📱 Platform-Specific Guidelines

### iOS Requirements
- Support iOS 17+ with backwards compatibility considerations
- Implement Share Extension for URL capture
- Use Face ID/Touch ID for biometric authentication
- Support Dynamic Type and accessibility features
- Implement proper handling of app lifecycle events

### macOS Requirements
- Support macOS 14+ (Sonoma and later)
- Implement native macOS interactions (right-click menus, keyboard shortcuts)
- Use AppKit integrations where appropriate
- Support multiple windows and proper window management
- Implement proper menu bar integration

## 🚀 Performance Optimization Rules

### SwiftData Optimization
```swift
// ✅ CORRECT - Use optimized query descriptors
let descriptor = LinkQueryDescriptors.filtered(
    searchTerm: searchText,
    status: "synced",
    sort: .recent,
    limit: 50
)

// ❌ WRONG - Unoptimized queries
@Query var allLinks: [LinkEntity]  // Loads everything!
```

### View Model Pattern
```swift
// ✅ CORRECT - Follow ViewModelProtocol
class FeatureViewModel: BaseViewModel {
    func performAction() async {
        setLoading(true)
        do {
            let result = try await service.performAction()
            // Handle success
        } catch {
            handleError(error)
        }
    }
}

// ❌ WRONG - Manual error handling
class BadViewModel: ObservableObject {
    @Published var error: String = ""
    // Manual loading state management
}
```

## 🔧 Development Workflow

### Before Making Changes
1. **Review existing patterns** in similar features
2. **Check Theme.swift** for existing design tokens
3. **Use platform components** from `Shared/Components/`
4. **Write tests first** for new functionality
5. **Consider offline scenarios** in implementation

### File Creation Guidelines
1. **Place in correct feature folder** according to architecture
2. **Use appropriate file naming** (ViewModels end with "ViewModel")
3. **Import only necessary modules** to reduce compilation time
4. **Add comprehensive documentation** for public interfaces
5. **Include unit tests** in corresponding test target

### Code Review Checklist
- [ ] Follows feature-based architecture
- [ ] Uses platform components for cross-platform code
- [ ] Applies theme system consistently
- [ ] Includes comprehensive unit tests
- [ ] Handles offline scenarios appropriately
- [ ] Uses optimized SwiftData queries
- [ ] Implements proper error handling
- [ ] Follows ViewModelProtocol pattern

## 🎯 Agent-Specific Instructions

### When Adding New Features
1. **Create in appropriate Features/ subdirectory**
2. **Extend theme system** if new design tokens needed
3. **Create view model** inheriting from BaseViewModel
4. **Use optimized query descriptors** for data access
5. **Write comprehensive unit tests** with offline scenarios
6. **Implement platform-specific behavior** using existing components

### When Modifying Existing Code
1. **Maintain backwards compatibility** where possible
2. **Update tests** to reflect changes
3. **Check cross-platform behavior** on both iOS and macOS
4. **Verify offline functionality** still works
5. **Update documentation** if public interfaces change

### When Debugging Issues
1. **Check offline/online state** first
2. **Verify SwiftData query efficiency** with large datasets
3. **Test on both iOS and macOS** platforms
4. **Review error handling** for user-friendly messages
5. **Validate security implementation** for sensitive operations

## 🚨 Critical Warnings

### NEVER Do These Things
- ❌ Store sensitive data in UserDefaults
- ❌ Make API calls without App Attestation headers
- ❌ Create platform-specific UI code outside Platform/ directory
- ❌ Use UIKit for UI development (SwiftUI only, except for approved platform abstractions)
- ❌ Skip unit tests for new functionality
- ❌ Use magic numbers instead of Theme system
- ❌ Implement local data persistence outside SwiftData cache
- ❌ Create unoptimized SwiftData queries
- ❌ Ignore offline scenarios in implementation

### ALWAYS Do These Things
- ✅ Use hamrah-api for all data persistence
- ✅ Implement offline-first functionality
- ✅ Follow the established architecture
- ✅ Write comprehensive unit tests
- ✅ Use platform components for cross-platform code
- ✅ Apply theme system for consistent styling
- ✅ Optimize performance for large datasets
- ✅ Handle errors gracefully with user feedback

## 📚 Related Documentation

- **API Documentation**: `../hamrah-api/README.md`
- **Web App**: `../hamrah-app/README.md`
- **Improvements Summary**: `IMPROVEMENTS_SUMMARY.md`
- **Testing Guidelines**: `hamrah-iosTests/README.md`

## 🎯 Success Metrics

A successful implementation must:
- ✅ Compile and run on both iOS and macOS without errors
- ✅ Pass all unit tests with 100% coverage
- ✅ Function completely offline with local data
- ✅ Sync seamlessly when internet is available
- ✅ Provide excellent performance with large datasets
- ✅ Follow all security best practices
- ✅ Maintain consistent UI/UX across platforms

---

**Remember**: This is a production application with real users. Code quality, security, and performance are not optional - they are requirements for every line of code.
