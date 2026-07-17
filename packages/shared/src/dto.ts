/**
 * Shared Data Transfer Objects (DTOs) for hamrah-api <-> clients.
 *
 * Wire protocol rules (NON-NEGOTIABLE):
 * - All JSON keys MUST be snake_case.
 * - All timestamps MUST be RFC 3339 (ISO 8601) UTC strings.
 * - Error shape: { success: false, error: string }
 *
 * This file defines strictly typed interfaces for:
 * - User representations
 * - Session validation
 * - Token issuance / refresh
 * - WebAuthn flows
 * - OAuth / native authentication
 * - Generic API envelopes
 *
 * Internal client code may map these wire shapes to richer domain models;
 * keep all wire boundary types here to ensure consistency across packages.
 */

/* ---------------------------------- Users ---------------------------------- */

/**
 * Canonical wire representation of a user from hamrah-api.
 */
export interface ApiUserWire {
  id: string;
  email: string;
  name?: string | null;
  picture?: string | null;
  provider?: string | null;
  auth_method?: string | null;
  provider_id?: string | null;
  created_at: string;
  updated_at?: string;
  last_login_at?: string | null;
  last_login_platform?: string | null;
  email_verified_at?: string | null;
}

/**
 * Internal (optional) parsed user shape.
 * NOTE: Kept here for convenience; do NOT send this over the wire.
 * Field names intentionally stay snake_case to match the auth wire contract.
 */
export interface AppUser {
  id: string;
  email: string;
  name?: string | null;
  picture?: string | null;
  provider?: string | null;
  auth_method?: string | null;
  provider_id?: string | null;
  created_at: Date;
  updated_at?: Date;
  last_login_at?: Date | null;
  last_login_platform?: string | null;
  email_verified_at?: Date | null;
}

/* -------------------------- API Response Envelopes ------------------------- */

export interface ApiSuccess<T> {
  success: true;
  data: T;
}

export interface ApiFailure {
  success: false;
  error: string;
}

export type ApiResult<T> = ApiSuccess<T> | ApiFailure;

/**
 * Standard user wrapper response.
 */
export interface UserResponseWire {
  success: boolean;
  user?: ApiUserWire;
  error?: string;
}

/* ----------------------------- Session Validation -------------------------- */

export interface SessionValidationResponse {
  success: boolean;
  user?: ApiUserWire;
  // session_token may or may not be returned depending on endpoint semantics
  session_token?: string;
  expires_at?: string;
  error?: string;
}

/* ---------------------------------- Tokens --------------------------------- */

/**
 * Token issuance / refresh response from hamrah-api.
 */
export interface TokenIssueResponse {
  success: boolean;
  access_token?: string;
  refresh_token?: string;
  expires_in?: number; // seconds until access token expiry
  error?: string;
}

export interface TokenRefreshRequest {
  refresh_token: string;
}

/* --------------------------------- WebAuthn -------------------------------- */

export interface RegisterBeginRequest {
  user_id: string;
  email: string;
  display_name?: string;
}

export interface RegisterBeginResponse {
  success: boolean;
  options?: unknown; // PublicKeyCredentialCreationOptions (JSON)
  challenge_id?: string;
  error?: string;
}

export interface RegisterVerifyRequest {
  challenge_id: string;
  response: unknown; // Credential response JSON from @simplewebauthn/browser
}

export interface RegisterVerifyResponse {
  success: boolean;
  credential_id?: string;
  error?: string;
}

export interface AuthBeginDiscoverableRequest {
  explicit?: boolean;
}

export interface AuthBeginDiscoverableResponse {
  success: boolean;
  options?: unknown; // PublicKeyCredentialRequestOptions (JSON)
  challenge_id?: string;
  error?: string;
}

export interface AuthVerifyDiscoverableRequest {
  challenge_id: string;
  response: unknown;
  mode?: "discoverable-explicit" | "discoverable-implicit";
}

export interface AuthVerifyDiscoverableResponse {
  success: boolean;
  user?: ApiUserWire;
  session_token?: string;
  access_token?: string;
  refresh_token?: string;
  expires_in?: number;
  expires_at?: string;
  error?: string;
}

/* ------------------------------ Native Auth Flow --------------------------- */

export interface NativeAuthRequest {
  provider?: "apple" | "google" | "webauthn";
  credential?: string; // ID token
  email?: string;
  name?: string;
  picture?: string;
  provider_id?: string;
  auth_method?: string;
  platform?: "web" | "ios" | "android" | "api";
  client_attestation?: string;
  email_verified_at?: string;
}

export interface NativeAuthResponse {
  success: boolean;
  user?: ApiUserWire;
  access_token?: string;
  refresh_token?: string;
  expires_in?: number;
  expires_at?: string;
  error?: string;
}

/* ------------------------------- Music sync -------------------------------- */

export type MusicProvider = "spotify" | "tidal";

export interface MusicConnectionWire {
  provider: MusicProvider;
  provider_account_id?: string | null;
  status: "pending" | "connected" | "error";
  connected_at?: string | null;
  last_error?: string | null;
}

export interface MusicImportRequestWire {
  include_owned_playlists: boolean;
  include_saved_playlists: boolean;
  include_followed_artists: boolean;
}

export interface MusicImportWire {
  id: string;
  status: "queued" | "running" | "completed" | "partial" | "failed";
  include_owned_playlists: boolean;
  include_saved_playlists: boolean;
  include_followed_artists: boolean;
  stage:
    | "queued"
    | "preparing"
    | "reading_spotify"
    | "creating_playlists"
    | "matching_artists"
    | "following_artists"
    | "completed"
    | "failed";
  total_items: number;
  imported_items: number;
  unmatched_items: number;
  playlist_total: number;
  playlists_imported: number;
  artist_total: number;
  artists_checked: number;
  artists_matched: number;
  artists_followed: number;
  error?: string | null;
  created_at: string;
}

/* --------------------------------- Utilities -------------------------------- */

/**
 * Convert wire user to internal AppUser (optional usage).
 */
export function mapApiUserWireToAppUser(user: ApiUserWire): AppUser {
  return {
    id: user.id,
    email: user.email,
    name: user.name ?? null,
    picture: user.picture ?? null,
    provider: user.provider ?? null,
    auth_method: user.auth_method ?? null,
    provider_id: user.provider_id ?? null,
    created_at: new Date(user.created_at),
    updated_at: user.updated_at ? new Date(user.updated_at) : undefined,
    last_login_at: user.last_login_at ? new Date(user.last_login_at) : null,
    last_login_platform: user.last_login_platform ?? null,
    email_verified_at: user.email_verified_at
      ? new Date(user.email_verified_at)
      : null,
  };
}

/**
 * Narrow an ApiResult and extract data or throw with normalized error.
 */
export function unwrapResult<T>(result: ApiResult<T>, context: string): T {
  if (!result.success) {
    throw new Error(`${context} failed: ${result.error}`);
  }
  return result.data;
}

/**
 * Type guard helpers.
 */
export function isApiSuccess<T>(r: ApiResult<T>): r is ApiSuccess<T> {
  return r.success;
}

export function isApiFailure<T>(r: ApiResult<T>): r is ApiFailure {
  return !r.success;
}

/**
 * Safe date parser from wire string (returns undefined if invalid).
 */
export function parseWireDate(value?: string | null): Date | undefined {
  if (!value) return undefined;
  const d = new Date(value);
  return isNaN(d.getTime()) ? undefined : d;
}

/* ---------------------------------- Errors ---------------------------------- */

/**
 * Standardized error factory for client-side consumption.
 */
export function apiError(message: string): ApiFailure {
  return { success: false, error: message };
}

/**
 * Build success envelope.
 */
export function apiSuccess<T>(data: T): ApiSuccess<T> {
  return { success: true, data };
}

/* --------------------------- Future Extension Notes ------------------------ */
/**
 * Potential future shared additions:
 * - Pagination metadata interfaces
 * - Link / content entity DTOs (once API endpoints are defined)
 * - Batch operation envelope types
 * - Conflict resolution payload structures for offline sync
 *
 * Keep this file focused: user/session/auth/webAuthn only.
 * Add new thematic DTO files (e.g., links.ts) instead of expanding indefinitely.
 */
