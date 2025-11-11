# hamrah-shared

Shared TypeScript DTOs and utilities for hamrah-api clients (web, iOS, future services).
This package centralizes the wire-level types and mapping helpers to keep all projects aligned.

## Goals

- Single source of truth for API contracts
- Enforce snake_case JSON keys across all requests and responses
- RFC 3339 UTC timestamp strings at the wire boundary
- Eliminate ad-hoc any typing and scattered model definitions
- Make refactors safer via shared, reusable types

## Status

- Intended for local workspace consumption (not published externally)
- Lives in the monorepo at: `packages/shared`
- Initial module: `src/dto.ts` (users, sessions, tokens, WebAuthn, native auth)

## Design Rules (NON-NEGOTIABLE)

1. Wire protocol JSON keys MUST be snake_case
2. Timestamps MUST be RFC 3339 strings in UTC (e.g., 2025-10-11T03:45:00Z)
3. Error responses MUST include `{ success: false, error: string }`
4. Clients may optionally map to internal camelCase shapes, but never send camelCase on the wire
5. Avoid any where possible; use DTOs and adapters

## Package Layout

```
packages/shared/
├── README.md         # This file
└── src/
    └── dto.ts        # Shared wire DTOs + mapping helpers
```

## Usage

Import wire DTOs directly where you integrate with hamrah-api. For example (in the web client):

```ts
// packages/web/src/lib/api/example.ts
import type { SessionValidationResponse, ApiUserWire } from '@hamrah/shared/dto';
// or use a relative path if you haven't added a package.json for shared yet:
// import type { SessionValidationResponse, ApiUserWire } from '../../shared/src/dto';

async function fetchSession(): Promise<SessionValidationResponse> {
  const resp = await fetch('https://api.hamrah.app/api/auth/sessions/validate', {
    credentials: 'include',
    headers: { 'content-type': 'application/json' },
  });
  return resp.json();
}

export async function getCurrentUser(): Promise<ApiUserWire | null> {
  const result = await fetchSession();
  return result.success ? result.user ?? null : null;
}
```

Optionally map to internal app models if you prefer:

```ts
import { mapApiUserWireToAppUser, type AppUser } from '@hamrah/shared/dto';

export async function getAppUser(): Promise<AppUser | null> {
  const resp = await fetch('/api/auth/user');
  const data = await resp.json();
  if (!data?.user) return null;
  return mapApiUserWireToAppUser(data.user);
}
```

## Included DTOs (Initial Set)

- Users: `ApiUserWire`, `AppUser` (internal optional)
- Sessions: `SessionValidationResponse`
- Tokens: `TokenIssueResponse`, `TokenRefreshRequest`
- WebAuthn: register/authenticate begin/verify request/response DTOs
- Native auth (Apple/Google): `NativeAuthRequest`, `NativeAuthResponse`
- Helpers: `apiSuccess`, `apiError`, `unwrapResult`, `parseWireDate`, type guards

See `src/dto.ts` for exact shapes.

## Conventions and Patterns

- Keep DTOs minimal and strictly represent wire shapes (snake_case).
- Add mapping helpers when you need ergonomic app models.
- Never add business logic here—only types and pure helpers.
- When adding a new entity:
  - Define the `*Wire` interface reflecting the API JSON payloads (snake_case).
  - If the client needs a camelCase version, add a pure mapping function.
  - Include request/response envelopes when appropriate.
  - Add comments linking to the API endpoint(s) that use the types.

## How to Add This Package as a Workspace Dependency

1. Create `packages/shared/package.json` (if not present) with:
   - `"name": "@hamrah/shared"`
   - `"type": "module"`
   - `"main": "src/dto.ts"`
   - `"types": "src/dto.ts"`
2. Reference it in other workspaces (web/api) via `@hamrah/shared`.
3. Ensure `pnpm-workspace.yaml` includes `packages/shared`.

Example `package.json` (suggested):

```json
{
  "name": "@hamrah/shared",
  "private": true,
  "version": "0.0.0",
  "type": "module",
  "main": "src/dto.ts",
  "types": "src/dto.ts",
  "license": "MIT"
}
```

Then in the consumer package:

```json
{
  "dependencies": {
    "@hamrah/shared": "workspace:*"
  }
}
```

## Versioning Guidance

Since this is a monorepo-internal package:
- Use `workspace:*` and keep versions in sync.
- Treat breaking changes seriously:
  - Update all consumers in the same PR.
  - Keep DTO changes backward-compatible when possible (additive changes, optional fields).
  - Document migrations in a brief section below or in the parent repo’s MIGRATION docs.

## Example API Contracts

- Session validation:
  - Request: GET `/api/auth/sessions/validate`
  - Response: `SessionValidationResponse`
- Token issuance:
  - Request: POST `/api/auth/token` with `{ refresh_token }`
  - Response: `TokenIssueResponse`
- WebAuthn (discoverable):
  - Begin: POST `/api/webauthn/authenticate/discoverable`
  - Verify: POST `/api/webauthn/authenticate/discoverable/verify`
- Native auth:
  - Request: POST `/api/auth/native`
  - Response: `NativeAuthResponse`

All keys snake_case. All timestamps RFC 3339 UTC.

## Adding New DTOs Checklist

- Define `*Wire` interfaces for request/response payloads.
- Add domain mapping helpers only if necessary.
- Add JSDoc comments with endpoint references.
- Keep names explicit and unambiguous.
- Ensure error envelope consistency: `{ success: false, error: string }`.
- Do not import app-specific or platform-specific libraries here.

## FAQ

- Why snake_case here but camelCase in code?
  - The wire protocol is standardized to snake_case across all platforms; internal models can be idiomatic per language/environment. Use the provided mappers when needed.
- Can I put business logic in shared?
  - No. Keep this package strictly about types, contracts, and pure transformations.

## License

MIT
