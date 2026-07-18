# hamrah-web Agents and Style Guide

This document provides a concise style guide for hamrah-web, focusing on API interactions and data shape consistency across the app.

## JSON and Data Shape

- All JSON keys exchanged with the API MUST be snake_case (requests and responses).
- Do not support or emit dual-case keys (no camelCase fallbacks).
- If a third-party API returns camelCase, normalize to snake_case at the boundary before it enters app state.

Recommended approach:

- Define TypeScript interfaces using snake_case to mirror the wire format.
- Perform any mapping at the integration boundary if unavoidable (e.g., external SDKs).

Example:

```ts
type RegisterBeginRequest = {
  user_id: string;
  email: string;
  display_name?: string;
  label?: string;
  flow_id?: string;
};

type RegisterBeginResponse = {
  success: boolean;
  options?: any; // WebAuthn CreationOptions JSON
  challenge_id?: string;
  error?: string;
};
```

## WebAuthn

Use snake_case for all WebAuthn request/response fields.

Requests:

- Register begin: user_id, email, display_name, label (optional), flow_id (optional)
- Register verify: challenge_id, response, label (optional), flow_id (optional)
- Authenticate begin (discoverable): flow_id (optional)
- Authenticate verify (discoverable): challenge_id, response

Responses:

- Register begin: success, options, challenge_id, error
- Register verify: success, credential_id, error
- Authenticate begin: success, challenge_id, options, error
- Authenticate verify: success, user, session_token (if applicable), error

## Auth and Sessions

- Token responses: access_token, refresh_token, expires_in
- Session info: session_token (string), and session objects use expires_at
- Validation results returned or used within the app should prefer is_valid over isValid
- Web auth is cookie-session based. Do not store bearer, access, refresh, or session tokens in localStorage or sessionStorage.
- Feature/domain code must call `HamrahApiClient` or an auth/domain wrapper that delegates to it; do not use raw `fetch` for hamrah-api calls.
- Use request auth policies `auth: "none" | "optional" | "required"` to describe whether a session is needed. The client owns SSR cookie forwarding, `credentials: "include"`, CSRF headers for unsafe methods, JSON parsing, and typed errors.
- Standard API error categories are `unauthorized`, `session_expired`, `server`, `network`, and `decoding`.

Example session result shape:

```ts
type SessionValidationResult = {
  success: boolean;
  user?: any;
  session?: { token: string; expires_at: Date } | null;
  is_valid: boolean;
  error?: string;
};
```

## Headers, Paths, and Query Params

- Custom HTTP headers: use lowercase, hyphen-separated (e.g., x-user-id, x-trace-id). Header names are case-insensitive on the wire.
- Path parameters: snake_case (e.g., /api/webauthn/users/{user_id}/credentials)
- Query parameters: snake_case (e.g., page_size, created_before)

## Timestamps

- Use RFC 3339 (ISO 8601) in UTC in payloads (e.g., created_at, updated_at, last_used, expires_at).
- In the web app, convert to `Date` where needed at usage boundaries.

## Testing and Fixtures

- Update mocks, fixtures, and e2e tests to use snake_case exclusively (e.g., challenge_id, credential_id).
- Avoid compatibility branches like `foo || fooAlt`; remove dual-case fallback logic.
- Before changing a user workflow, inspect the matching iOS/macOS implementation and shared API contract. Web-only completion is valid only when the sibling client is verified unaffected and that evidence is recorded in the PR.
- Interaction bugs require a browser/component test that exercises the control, state transition, loading/error feedback, and emitted API payload. A pure helper test alone does not prove the web workflow.
- Verify responsive behavior at mobile and desktop widths for changed interactive layouts.

## Migration Guidance

- When touching files that still use camelCase for wire data, refactor to snake_case and remove aliases/fallbacks.
- Component props and local variables may continue to follow typical TypeScript/JS conventions; the wire protocol and app state models should be snake_case.
- If consumers can’t be migrated at once, add a temporary adapter at the boundary and plan removal.

## Summary

- Wire protocol (to/from API) is strictly snake_case.
- No dual-case support in the web app.
- Normalize third-party or legacy camelCase data to snake_case at the edges.

## CORS and CSRF for Direct API Calls

When calling `https://api.hamrah.app` directly from the web app (e.g., session validation and logout), the following are REQUIRED:

### CORS (API → Browser)

- Access-Control-Allow-Origin: https://hamrah.app
  - Must be the exact origin. Do not use `*` when sending credentials.
- Access-Control-Allow-Credentials: true
- Access-Control-Allow-Methods: GET, POST, OPTIONS
- Access-Control-Allow-Headers: content-type, authorization
- Responses that clear or set the session cookie should also return a `Set-Cookie` header (see cookie attributes below).

Client requests MUST include credentials:

```ts
await fetch("https://api.hamrah.app/api/auth/sessions/logout", {
  method: "POST",
  credentials: "include",
  headers: { "Content-Type": "application/json" },
});
```

### Cookie Attributes (API sets/clears session)

- Set-Cookie: session=...; Domain=.hamrah.app; Path=/; Secure; HttpOnly; SameSite=Lax
  - Domain `.hamrah.app` ensures the cookie is sent to both `hamrah.app` and `api.hamrah.app`.
  - `HttpOnly` prevents JS access; browser will still attach it to requests.
  - `Secure` required for HTTPS.
  - `SameSite=Lax` is recommended for same-site subdomain usage (adjust only if needed for special flows).

### CSRF Protection (Double-Submit)

- Backend sets a non-HttpOnly CSRF cookie (e.g., `csrf_token`).
- Frontend echoes the value via a header on unsafe methods:
  - X-CSRF-Token: <value from csrf_token cookie>
- Keep using `credentials: 'include'` so the session cookie is attached by the browser.
- This approach complements same-site protections and Origin checks.

Example (header echo):

```ts
const csrf = document.cookie
  .split(";")
  .map((c) => c.trim())
  .find((c) => c.startsWith("csrf_token="))
  ?.split("=")
  .slice(1)
  .join("=");

await fetch("https://api.hamrah.app/api/auth/sessions/logout", {
  method: "POST",
  credentials: "include",
  headers: {
    "Content-Type": "application/json",
    ...(csrf ? { "X-CSRF-Token": decodeURIComponent(csrf) } : {}),
  },
});
```

### Manual Verification (curl)

```bash
curl -i -X POST https://api.hamrah.app/api/auth/sessions/logout \
  -H "Origin: https://hamrah.app" \
  -H "Content-Type: application/json" \
  --cookie-jar cookies.txt --cookie cookies.txt
```

Expect:

- Access-Control-Allow-Origin: https://hamrah.app
- Access-Control-Allow-Credentials: true
- Appropriate `Set-Cookie` clearing the session.
