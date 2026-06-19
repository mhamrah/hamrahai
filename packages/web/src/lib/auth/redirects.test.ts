import { describe, expect, it } from "vitest";
import { safeRedirectPath } from "./redirects";

describe("safeRedirectPath", () => {
  it("allows local redirects", () => {
    expect(safeRedirectPath("/settings")).toBe("/settings");
    expect(safeRedirectPath("%2Fsettings")).toBe("/settings");
  });

  it("rejects external redirects", () => {
    expect(safeRedirectPath("https://example.com")).toBe("/");
    expect(safeRedirectPath("//example.com")).toBe("/");
  });
});
