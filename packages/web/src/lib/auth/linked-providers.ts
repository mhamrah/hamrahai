import type { ApiUserWire } from "@hamrah/shared";

export function linkedAuthProviders(
  user: Pick<ApiUserWire, "provider" | "auth_providers">,
): ReadonlySet<string> {
  const providers = user.auth_providers?.length
    ? user.auth_providers
    : [user.provider];

  return new Set(
    providers
      .filter((provider): provider is string => Boolean(provider))
      .map((provider) => provider.toLowerCase()),
  );
}
