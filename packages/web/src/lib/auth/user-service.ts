// This file has been deprecated - user operations are now handled via hamrah-api
// All user creation and management operations should use the API client instead
// See ~/lib/auth/api-client.ts for the replacement functionality

export function findOrCreateUser(): any {
  throw new Error("findOrCreateUser has been moved to hamrah-api");
}