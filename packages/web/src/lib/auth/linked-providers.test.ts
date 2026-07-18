import { describe, expect, it } from "vitest";

import { linkedAuthProviders } from "./linked-providers";

describe("linkedAuthProviders", () => {
  it("returns every provider linked to the account", () => {
    const providers = linkedAuthProviders({
      provider: "apple",
      auth_providers: ["apple", "google"],
    });

    expect(providers).toEqual(new Set(["apple", "google"]));
  });

  it("uses the legacy last-used provider when the provider list is absent", () => {
    expect(linkedAuthProviders({ provider: "google" })).toEqual(
      new Set(["google"]),
    );
  });
});
