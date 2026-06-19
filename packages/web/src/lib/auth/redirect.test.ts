import { describe, expect, it } from "vitest";
import { getSafeRedirectPath } from "./redirect";

describe("getSafeRedirectPath", () => {
  it("allows internal paths with query strings and hashes", () => {
    expect(getSafeRedirectPath("/inbox?page=2#saved")).toBe(
      "/inbox?page=2#saved",
    );
  });

  it("falls back for missing redirects", () => {
    expect(getSafeRedirectPath(null)).toBe("/");
    expect(getSafeRedirectPath(undefined)).toBe("/");
    expect(getSafeRedirectPath("")).toBe("/");
  });

  it("rejects external and protocol-relative redirects", () => {
    expect(getSafeRedirectPath("https://evil.example/login")).toBe("/");
    expect(getSafeRedirectPath("//evil.example/login")).toBe("/");
  });

  it("rejects backslashes and control characters", () => {
    expect(getSafeRedirectPath("/\\evil.example")).toBe("/");
    expect(getSafeRedirectPath("/inbox\nSet-Cookie:bad")).toBe("/");
  });
});
