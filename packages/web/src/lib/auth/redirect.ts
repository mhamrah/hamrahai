const DEFAULT_REDIRECT_PATH = "/";

export function getSafeRedirectPath(
  redirect: string | null | undefined,
): string {
  if (!redirect) {
    return DEFAULT_REDIRECT_PATH;
  }

  const hasControlCharacters = /[\u0000-\u001F\u007F]/.test(redirect);
  const isInternalPath =
    redirect.startsWith("/") &&
    !redirect.startsWith("//") &&
    !redirect.includes("\\");

  return isInternalPath && !hasControlCharacters
    ? redirect
    : DEFAULT_REDIRECT_PATH;
}
