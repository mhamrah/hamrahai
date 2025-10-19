# Hamrah Monorepo

> AI-powered personal knowledge management across Web, iOS, and macOS

## Overview

Hamrah is a multi-platform application for organizing, saving, and retrieving information with AI-powered insights. This monorepo contains three integrated applications:

- **🦀 API** - Rust backend (Google Cloud Run)
- **🌐 Web** - Qwik frontend (Cloudflare Workers)  
- **📱 iOS** - Native Swift app (iOS & macOS)

## Quick Start

```bash
# Install dependencies
pnpm install

# Start web development
pnpm web:dev

# Run API tests
pnpm api:test

# Open iOS project
pnpm ios:open
```

## Repository Structure

```
hamrahai/
├── packages/
│   ├── api/          # Rust backend API
│   ├── web/          # Qwik web application
│   ├── ios/          # Swift iOS/macOS app
│   └── shared/       # (Future) Shared TypeScript types
├── .github/
│   └── workflows/    # CI/CD pipelines
├── AGENTS.md         # Comprehensive agent instructions
├── package.json      # Root workspace scripts
└── pnpm-workspace.yaml
```

## Package Overview

### 📦 API (Rust)

Backend service providing authentication, data persistence, and AI-powered features.

**Tech Stack:** Rust, Axum, PostgreSQL, SQLx  
**Deployment:** Google Cloud Run  
**Endpoint:** https://api.hamrah.app

```bash
cd packages/api
cargo build           # Build
cargo test            # Test
cargo fmt             # Format
cargo clippy -- -D warnings  # Lint
```

[Full API Documentation →](packages/api/README.md)

### 📦 Web (TypeScript/Qwik)

Web application with server-side rendering and progressive enhancement.

**Tech Stack:** Qwik, TypeScript, Tailwind CSS, Vite  
**Deployment:** Cloudflare Workers  
**URL:** https://hamrah.app

```bash
cd packages/web
pnpm dev              # Development server
pnpm build            # Production build
pnpm test:run         # Unit tests
pnpm test:e2e         # E2E tests
```

[Full Web Documentation →](packages/web/README.md)

### 📦 iOS (Swift)

Native iOS and macOS applications with offline-first architecture.

**Tech Stack:** SwiftUI, SwiftData, Xcode  
**Platforms:** iOS 17+, macOS 14+  
**Deployment:** App Store

```bash
pnpm ios:open         # Open in Xcode
# Then ⌘+B to build, ⌘+U to test
```

[Full iOS Documentation →](packages/ios/README.md)

## Development

### Prerequisites

- **Node.js** 18+ and **pnpm** 8+
- **Rust** (latest stable) and **Cargo**
- **Xcode** 15+ (for iOS development)
- **Docker** (for API containerization)

### Installation

```bash
# Clone repository
git clone git@github.com:yourusername/hamrahai.git
cd hamrahai

# Install Node dependencies
pnpm install
```

### Common Commands

```bash
# Web Development
pnpm web:dev          # Start dev server
pnpm web:build        # Build for production
pnpm web:test         # Run tests
pnpm web:lint         # Lint code

# API Development
pnpm api:build        # Build debug
pnpm api:test         # Run tests
pnpm api:fmt          # Format code
pnpm api:clippy       # Lint code
pnpm api:docker       # Build Docker image

# iOS Development
pnpm ios:open         # Open Xcode project

# Monorepo Operations
pnpm fmt              # Format all projects
pnpm lint             # Lint all projects
pnpm test             # Test all projects
```

## Deployment

### API → Cloud Run

**Trigger:** Push to `main` with changes in `packages/api/**`

Automated via GitHub Actions:
1. Build Docker image
2. Push to Google Artifact Registry
3. Deploy to Cloud Run
4. Health checks validate deployment

### Web → Cloudflare Workers

**Trigger:** Push to `main` with changes in `packages/web/**`

Automated via GitHub Actions:
1. Install dependencies
2. Build with Vite
3. Deploy via Wrangler to Cloudflare

### iOS → App Store

Manual deployment via Xcode:
1. Open project in Xcode
2. Archive build
3. Upload to App Store Connect

## Architecture

### System Overview

```
┌─────────────┐
│   iOS App   │────┐
└─────────────┘    │
                   │
┌─────────────┐    │    ┌──────────────┐      ┌────────────┐
│   Web App   │────┼───▶│  Rust API    │─────▶│ PostgreSQL │
└─────────────┘    │    │ (Cloud Run)  │      └────────────┘
                   │    └──────────────┘
┌─────────────┐    │
│  macOS App  │────┘
└─────────────┘
```

### Key Features

- **Offline-First**: All clients work without internet, sync when available
- **Multi-Platform**: Native experiences on Web, iOS, and macOS
- **AI-Powered**: Automatic content summarization and organization
- **Secure**: WebAuthn, OAuth, App Attestation, JWT tokens
- **Modern Stack**: Latest technologies and best practices

## API Contract

All projects communicate using snake_case JSON:

```typescript
// Example: Register WebAuthn credential
POST /api/webauthn/register/begin
{
  user_id: string;
  email: string;
  display_name?: string;
}

Response:
{
  success: boolean;
  challenge_id?: string;
  options?: object;
  error?: string;
}
```

See [AGENTS.md](AGENTS.md) for complete API documentation.

## Testing

Each package has comprehensive tests:

```bash
# API tests (Rust)
cd packages/api && cargo test

# Web tests (Vitest + Playwright)
cd packages/web && pnpm test:run && pnpm test:e2e

# iOS tests (XCTest)
# Open in Xcode and press ⌘+U
```

## CI/CD

GitHub Actions workflows are path-filtered to run only for changed packages:

- `api-deploy.yml` - Builds and deploys API to Cloud Run
- `web-ci.yml` - Tests and builds web application

Workflows automatically trigger on push to `main` or pull requests.

## Security

- **API**: Google Cloud Secret Manager for credentials
- **Web**: Cloudflare Workers secrets for sensitive data
- **iOS**: Keychain for secure storage, App Attestation for API calls
- **All**: Certificate pinning, JWT tokens, WebAuthn support

## Contributing

1. Make changes in appropriate `packages/` directory
2. Follow package-specific guidelines (see AGENTS.md)
3. Run tests and linters
4. Ensure builds succeed
5. Submit pull request

Detailed contribution guidelines in [AGENTS.md](AGENTS.md).

## Documentation

- **[AGENTS.md](AGENTS.md)** - Comprehensive development guide for all packages
- **[packages/api/README.md](packages/api/README.md)** - API documentation
- **[packages/web/README.md](packages/web/README.md)** - Web application docs
- **[packages/ios/README.md](packages/ios/README.md)** - iOS/macOS app docs

## Technology Stack

| Component | Technologies |
|-----------|-------------|
| **Backend** | Rust, Axum, Tokio, SQLx, PostgreSQL |
| **Web** | Qwik, TypeScript, Tailwind CSS, Vite, Vitest, Playwright |
| **iOS** | Swift, SwiftUI, SwiftData, Xcode |
| **Deployment** | Cloud Run, Cloudflare Workers, App Store |
| **CI/CD** | GitHub Actions |
| **Package Management** | pnpm workspaces, Cargo |

## License

MIT - See [LICENSE](LICENSE) for details

## Support

For questions or issues:
- Open an issue on GitHub
- Check package-specific documentation
- Review [AGENTS.md](AGENTS.md) for detailed guides

---

**Built with ❤️ for knowledge management across all your devices**
