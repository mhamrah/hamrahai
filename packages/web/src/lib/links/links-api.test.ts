import { describe, expect, it, vi } from "vitest";

import { archiveLink, deleteLink, listLinks } from "./links-api";

describe("links api", () => {
  it("lists active links and filters archived links from inbox results", async () => {
    const client = {
      get: vi.fn().mockResolvedValue({
        links: [
          {
            id: "active",
            original_url: "https://example.com/active",
            canonical_url: "https://example.com/active",
            tags: [],
            save_count: 1,
            status: "synced",
            shared_at: "2026-06-30T00:00:00Z",
            created_at: "2026-06-30T00:00:00Z",
            updated_at: "2026-06-30T00:00:00Z",
          },
          {
            id: "archived",
            original_url: "https://example.com/archived",
            canonical_url: "https://example.com/archived",
            tags: [],
            save_count: 1,
            status: "archived",
            shared_at: "2026-06-30T00:00:00Z",
            created_at: "2026-06-30T00:00:00Z",
            updated_at: "2026-06-30T00:00:00Z",
          },
          {
            id: "deleted",
            original_url: "https://example.com/deleted",
            canonical_url: "https://example.com/deleted",
            tags: [],
            save_count: 1,
            status: "deleted",
            shared_at: "2026-06-30T00:00:00Z",
            created_at: "2026-06-30T00:00:00Z",
            updated_at: "2026-06-30T00:00:00Z",
          },
        ],
        next_cursor: null,
      }),
    } as any;

    const links = await listLinks(undefined, client);

    expect(client.get).toHaveBeenCalledWith("/v1/links?since=&limit=100", {
      auth: "required",
    });
    expect(links.map((link) => link.id)).toEqual(["active"]);
  });

  it("archives through the persisted link status endpoint", async () => {
    const client = {
      patch: vi.fn().mockResolvedValue({
        success: true,
        link: { id: "link-1", status: "archived" },
      }),
    } as any;

    const link = await archiveLink("link-1", undefined, client);

    expect(client.patch).toHaveBeenCalledWith(
      "/v1/links/link-1",
      { status: "archived" },
      { auth: "required" },
    );
    expect(link.status).toBe("archived");
  });

  it("deletes through the persisted link delete endpoint", async () => {
    const client = {
      delete: vi.fn().mockResolvedValue({ success: true }),
    } as any;

    await expect(deleteLink("link-1", undefined, client)).resolves.toBe(true);

    expect(client.delete).toHaveBeenCalledWith("/v1/links/link-1", {
      auth: "required",
    });
  });
});
