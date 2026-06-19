export function safeRedirectPath(value: string | null | undefined): string {
  if (!value) return "/";

  try {
    const decoded = decodeURIComponent(value);
    if (!decoded.startsWith("/") || decoded.startsWith("//")) return "/";
    return decoded;
  } catch {
    return "/";
  }
}
