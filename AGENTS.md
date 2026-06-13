# Hamrah Monorepo - Agent Instructions

## Overview

Hamrah is a multi-platform AI-powered personal knowledge management system consisting of three integrated applications:

- **API** (Rust): Backend service deployed on Google Cloud Run
- **Web** (Qwik/TypeScript): Frontend web application deployed on Cloudflare Workers
- **iOS** (Swift): Native iOS and macOS applications

This monorepo enables efficient cross-platform development while maintaining independent build and deployment pipelines for each project.

---

## 🏗️ Monorepo Structure

```
hamrahai/
├── packages/
│   ├── api/          # Rust backend API
│   ├── web/          # Qwik web application
│   ├── ios/          # Swift iOS/macOS app
│   └── shared/       # (Future) Shared TypeScript types
├── .github/
│   └── workflows/    # CI/CD pipelines
├── AGENTS.md         # This file
├── package.json      # Root workspace configuration
└── pnpm-workspace.yaml
```

---

## 🎯 Core Principles

### Cross-Project Standards

1. **API Communication**: All data exchange uses snake_case JSON keys
2. **Authentication**: JWT tokens, WebAuthn passkeys, OAuth (Apple/Google)
3. **Offline-First**: All clients support offline functionality with server sync
4. **Security**: App Attestation, certificate pinning, secure token storage
5. **Testing**: Comprehensive unit and integration test coverage required

### Development Workflow

- Always create a pull request when submitting code changes for review. Do not leave code changes only as local edits unless the user explicitly asks not to open a PR.

```bash
# Work on specific packages
pnpm web:dev          # Start web dev server
pnpm api:test         # Run API tests
pnpm ios:open         # Open Xcode project

# Run from package directory
cd packages/web && pnpm dev
cd packages/api && cargo build
```

---

## 📦 Package: API (Rust Backend)

### Technology Stack
- **Framework**: Axum (async web framework)
- **Runtime**: Tokio (async runtime)
- **Database**: PostgreSQL with SQLx (compile-time checked queries)
- **Deployment**: Google Cloud Run (containerized native binary)
- **Auth**: JWT access/refresh tokens, WebAuthn, Apple/Google OAuth

### Architecture

Backend API for all Hamrah clients (iOS, macOS, Web). Designed for:
- Offline-first synchronization with conflict resolution
- AI-powered content summarization and organization
- Secure authentication with multiple providers
- RESTful endpoints for all client operations

### Key Components

1. **API Server**
   - Endpoint handling (auth, users, tokens, WebAuthn)
   - Data access with SQLx and PostgreSQL
   - JWT/session lifecycle management
   - Internal service endpoints for web app

2. **Link Pipeline**
   - URL ingestion and content extraction
   - AI summarization (provider-agnostic)
   - Metadata extraction and normalization
   - Offline-ready content storage

3. **Auth System**
   - OAuth integration (Apple, Google)
   - WebAuthn passwordless authentication
   - JWT token management with refresh rotation
   - Session validation and App Attestation

### Development Standards

**Code Quality:**
```bash
cargo fmt              # Format code (run after every change)
cargo clippy -- -D warnings  # Lint with warnings as errors
cargo test             # Run all tests
```

**API Conventions:**
- Use snake_case for all JSON keys (requests and responses)
- Axum route syntax: `{param}` for path parameters (e.g., `/api/users/{id}`)
- All timestamps in RFC 3339 (ISO 8601) UTC format
- Consistent error responses with `success`, `error` fields

**Deployment:**
- Endpoint: `https://api.hamrah.app`
- Secrets: `DATABASE_URL`, `JWT_SECRET` (Google Cloud Secret Manager)
- Port: 8080
- Health checks: `/healthz`, `/readyz`

### File Structure
```
packages/api/
├── Cargo.toml           # Rust dependencies
├── Dockerfile           # Cloud Run container
├── migrations/          # Database migrations
├── src/
│   ├── main.rs         # Application entry point
│   ├── auth/           # Authentication modules
│   ├── handlers/       # Route handlers
│   ├── models/         # Data models
│   └── services/       # Business logic
└── README.md
```

---

## 🌐 Package: Web (Qwik/TypeScript Frontend)

### Technology Stack
- **Framework**: Qwik (resumable framework)
- **Language**: TypeScript
- **Styling**: Tailwind CSS v4
- **Testing**: Vitest (unit), Playwright (e2e)
- **Deployment**: Cloudflare Workers
- **Package Manager**: pnpm

### Architecture

Web application providing browser access to Hamrah functionality. Features:
- Server-side rendering with Qwik
- Progressive enhancement
- API integration via HTTPS (no service bindings)
- WebAuthn authentication
- Responsive design for desktop and mobile

### Development Standards

**Code Quality:**
```bash
pnpm lint              # ESLint
pnpm fmt               # Prettier formatting
pnpm build.types       # TypeScript type checking
pnpm test:run          # Unit tests
pnpm test:e2e          # End-to-end tests
```

**JSON and Data Shape:**
- **CRITICAL**: All JSON keys MUST be snake_case (requests and responses)
- No dual-case support (no camelCase fallbacks)
- TypeScript interfaces mirror wire format with snake_case
- Normalize third-party camelCase at integration boundaries

**Example:**
```typescript
type RegisterBeginRequest = {
  user_id: string;
  email: string;
  display_name?: string;
};

type RegisterBeginResponse = {
  success: boolean;
  options?: any;
  challenge_id?: string;
  error?: string;
};
```

**API Integration:**
- All requests to `https://api.hamrah.app`
- Include proper headers: `Content-Type: application/json`
- Handle errors gracefully with user feedback
- Use snake_case for all request/response data

**Naming Conventions:**
- HTTP headers: lowercase, hyphen-separated (e.g., `x-user-id`)
- Path parameters: snake_case (e.g., `/api/users/{user_id}`)
- Query parameters: snake_case (e.g., `page_size`, `created_before`)
- Timestamps: RFC 3339 in UTC (e.g., `created_at`, `expires_at`)

**Deployment:**
- Domain: `https://hamrah.app`
- Configuration: `wrangler.jsonc`
- Environment: Cloudflare Workers (edge runtime)

### File Structure
```
packages/web/
├── package.json         # Dependencies and scripts
├── wrangler.jsonc       # Cloudflare Workers config
├── vite.config.ts       # Build configuration
├── src/
│   ├── routes/         # Page routes
│   ├── components/     # UI components
│   └── services/       # API client services
├── tests-e2e/          # Playwright tests
└── README.md
```

---

## 📱 Package: iOS (Swift Native App)

### Technology Stack
- **UI Framework**: SwiftUI (100% SwiftUI, no UIKit for UI)
- **Data**: SwiftData with App Group sharing
- **Platforms**: iOS 17+, macOS 14+
- **Auth**: Apple Sign-In, Google Sign-In, WebAuthn
- **Security**: Keychain, App Attestation, Biometric auth

### Architecture

Native Swift application for iOS and macOS providing offline-first experience with full-featured mobile access to Hamrah. Key features:
- Cross-platform (iOS and macOS) with shared codebase
- Offline-first with local SwiftData cache
- Background sync with hamrah-api
- Share Extension for URL capture
- Native authentication (Face ID/Touch ID)

### Core Principles (NON-NEGOTIABLE)

1. **Performance & Architecture**
   - Performant, well-modularized code for iOS and macOS
   - 100% unit test coverage for new functionality
   - Offline-first approach - must work without internet
   - All data persistence via hamrah-api (local cache only)

2. **Code Quality**
   - Modular feature-based architecture
   - SwiftUI-only for UI (UIKit only for platform abstractions)
   - Theme system for consistent styling
   - Type-safe Swift patterns
   - Optimized queries with pagination

3. **Security (CRITICAL)**
   - NEVER store sensitive data in UserDefaults (Keychain only)
   - ALWAYS include App Attestation headers
   - Implement biometric auth for sensitive operations
   - Certificate pinning for API calls
   - Validate all API responses

### Architecture Structure
```
packages/ios/
├── Core/
│   ├── Data/              # SwiftData query descriptors
│   ├── Managers/          # Business logic coordinators
│   ├── Models/            # SwiftData models
│   ├── Protocols/         # Standardized interfaces
│   └── Services/          # API, Keychain, Security
├── Features/
│   ├── Authentication/    # Login, biometric, OAuth
│   ├── Inbox/            # Link management UI
│   ├── Settings/         # App configuration
│   └── ShareExtension/   # Share handling
├── Shared/
│   ├── Components/       # Cross-platform UI
│   ├── Theme/            # Design system
│   └── Utilities/        # Helper functions
└── Platform/
    ├── iOS/              # iOS-specific code
    └── macOS/            # macOS-specific code
```

### SwiftUI-Only Development

**✅ ALLOWED UIKit Usage (Very Limited):**
- `PlatformBridge.swift` - Platform abstraction utilities
- `NativeAuthManager.swift` - Authentication contexts
- `AppAttestationManager.swift` - iOS App Attestation APIs
- `WebView` wrappers - UIViewRepresentable/NSViewRepresentable

**❌ PROHIBITED UIKit Usage:**
- `UIAlertController` - Use SwiftUI alerts
- `UIActivityViewController` - Use SwiftUI `ShareLink`
- `UIApplication.shared.open()` - Use `@Environment(\.openURL)`
- `UIPasteboard` - Use `PlatformBridge.copyToClipboard()`
- Direct `UIColor` - Use SwiftUI `Color(.systemBackground)`

### Development Standards

**Use Platform Components:**
```swift
// ✅ CORRECT
PlatformButton("Save", systemImage: "checkmark", style: .primary) {
    viewModel.save()
}

// ❌ WRONG
#if os(iOS)
Button("Save") { }.buttonStyle(.borderedProminent)
#endif
```

**Use Theme System:**
```swift
// ✅ CORRECT
Text("Title")
    .font(Theme.Typography.cardTitle)
    .foregroundColor(Theme.Colors.primaryText)
    .themedCard()

// ❌ WRONG
Text("Title")
    .font(.system(size: 18, weight: .medium))
    .padding(16)
```

**Optimized SwiftData Queries:**
```swift
// ✅ CORRECT - Use query descriptors
let descriptor = LinkQueryDescriptors.filtered(
    searchTerm: searchText,
    status: "synced",
    sort: .recent,
    limit: 50
)

// ❌ WRONG - Loads everything
@Query var allLinks: [LinkEntity]
```

**ViewModel Pattern:**
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
```

### Testing Requirements

- 100% unit test coverage for ViewModels
- 100% coverage for Core services and managers
- Test offline scenarios and sync conflicts
- Mock all API dependencies

### Performance Requirements

- Pagination: max 50 items initial load
- Debounced search: 300ms minimum delay
- Lazy loading for visible content only
- Proper memory cleanup (cancellables)

### Platform-Specific Guidelines

**iOS:**
- Support iOS 17+ with backwards compatibility
- Share Extension for URL capture
- Face ID/Touch ID biometric auth
- Dynamic Type and accessibility support

**macOS:**
- Support macOS 14+ (Sonoma+)
- Native interactions (right-click, keyboard shortcuts)
- Multiple window support
- Menu bar integration

---

## 🔄 Cross-Project Integration

### Shared API Contracts

All projects communicate with the API using consistent snake_case JSON:

**Authentication:**
```typescript
// Register begin
POST /api/webauthn/register/begin
{
  user_id: string;
  email: string;
  display_name?: string;
}

Response: {
  success: boolean;
  challenge_id?: string;
  options?: object;
  error?: string;
}
```

**Token Management:**
```typescript
POST /api/auth/token
Response: {
  access_token: string;
  refresh_token: string;
  expires_in: number;
}
```

### Shared Development Standards

1. **All timestamps**: RFC 3339 (ISO 8601) in UTC
2. **All JSON keys**: snake_case (no camelCase)
3. **Error responses**: Include `success: false` and `error` message
4. **Authentication**: JWT access tokens in `Authorization: Bearer` header
5. **API base URL**: `https://api.hamrah.app`

---

## 🚀 Deployment

### API Deployment (Cloud Run)

**Trigger:** Push to `main` with changes in `packages/api/**`

**Process:**
1. Build Docker image from `packages/api/Dockerfile`
2. Push to Google Artifact Registry
3. Deploy to Cloud Run service `hamrah-api`
4. Health checks on `/healthz` and `/readyz`

**Secrets:** Managed in Google Cloud Secret Manager

### Web Deployment (Cloudflare Workers)

**Trigger:** Push to `main` with changes in `packages/web/**`

**Process:**
1. Install dependencies with pnpm
2. Build with Vite
3. Deploy to Cloudflare Workers via wrangler
4. Live at `https://hamrah.app`

**Configuration:** `packages/web/wrangler.jsonc`

### iOS Deployment (App Store)

**Manual process via Xcode:**
1. Open `packages/ios/hamrah-ios.xcodeproj`
2. Archive build
3. Upload to App Store Connect
4. TestFlight or Production release

---

## 🧪 Testing Strategy

### API Tests
```bash
cd packages/api
cargo test                    # All tests
cargo test --test integration # Integration tests only
```

### Web Tests
```bash
cd packages/web
pnpm test:run                 # Unit tests
pnpm test:e2e                 # E2E tests with Playwright
pnpm test:coverage            # Coverage report
```

### iOS Tests
- Open Xcode project
- ⌘+U to run all tests
- View coverage in Xcode's coverage report

---

## 📝 Development Workflow

### Starting Development

```bash
# Clone and setup
git clone git@github.com:yourusername/hamrahai.git
cd hamrahai
pnpm install

# Start web development
pnpm web:dev

# Work on API
cd packages/api
cargo build
cargo test

# Open iOS project
pnpm ios:open
```

### Making Changes

1. **Work in appropriate package directory**
2. **Follow project-specific standards** (see sections above)
3. **Run tests** before committing
4. **Format code** with project formatters
5. **Update documentation** if changing public interfaces

### Before Committing

```bash
# Format all code
pnpm fmt

# Run linters
pnpm lint

# Run all tests
pnpm test
```

### CI/CD Behavior

- **Path-filtered**: Workflows only run for changed packages
- **API changes**: Trigger `api-deploy.yml`
- **Web changes**: Trigger `web-ci.yml`
- **Independent**: Each package deploys independently

---

## 🚨 Critical Rules

### NEVER Do These

- ❌ Store secrets in code or environment files committed to git
- ❌ Use camelCase for API request/response JSON keys
- ❌ Skip tests for new functionality
- ❌ Change deployment configs without testing
- ❌ Use UIKit for iOS UI development (except approved abstractions)
- ❌ Store sensitive data in UserDefaults (iOS) or localStorage (Web)
- ❌ Deploy without running full test suite

### ALWAYS Do These

- ✅ Use snake_case for all wire protocol JSON
- ✅ Include comprehensive unit tests
- ✅ Format code before committing
- ✅ Test offline scenarios (iOS app)
- ✅ Validate API responses
- ✅ Use established theme/component systems
- ✅ Handle errors gracefully with user feedback
- ✅ Test on both iOS and macOS (for iOS package)

---

## 📚 Package-Specific Documentation

Detailed documentation for each package:

- **API**: `packages/api/README.md`, `packages/api/DEPLOYMENT.md`
- **Web**: `packages/web/README.md`, `packages/web/AGENTS.md`
- **iOS**: `packages/ios/README.md`, `packages/ios/AGENTS.md`

---

## 🎯 Success Criteria for Changes

Any change must:

1. ✅ Build successfully in its package
2. ✅ Pass all existing tests
3. ✅ Include tests for new functionality
4. ✅ Follow project-specific style guidelines
5. ✅ Not break other packages
6. ✅ Work in offline scenarios (where applicable)
7. ✅ Maintain API contract compatibility
8. ✅ Include updated documentation if needed

---

## 🤝 Contributing

This monorepo enables seamless development across all Hamrah platforms. When adding features:

1. **Consider cross-platform impact** - API changes affect all clients
2. **Maintain API contracts** - snake_case JSON, consistent error handling
3. **Test thoroughly** - Each package has its own test requirements
4. **Document changes** - Update relevant package documentation
5. **Deploy safely** - CI/CD handles deployment automatically

**Remember**: This is a production application. Code quality, security, and performance are requirements, not suggestions.
