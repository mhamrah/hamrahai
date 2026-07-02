import type { RequestEventCommon } from "@builder.io/qwik-city";

import { createApiClient, type HamrahApiClient } from "~/lib/auth/api-client";

export type LinkStatus = "synced" | "archived" | "deleted" | string;

export interface ServerLink {
  id: string;
  original_url: string;
  canonical_url: string;
  title?: string | null;
  snippet?: string | null;
  summary_short?: string | null;
  summary_long?: string | null;
  lang?: string | null;
  tags: string[];
  save_count: number;
  status: LinkStatus;
  shared_at: string;
  created_at: string;
  updated_at: string;
}

export interface LinkDeltaResponse {
  links: ServerLink[];
  next_cursor?: string | null;
}

interface LinkMutationResponse {
  success: boolean;
  link: ServerLink;
}

interface DeleteLinkResponse {
  success: boolean;
}

type LinkApiClient = Pick<HamrahApiClient, "get" | "patch" | "delete">;

export async function listLinks(
  event?: RequestEventCommon,
  client: LinkApiClient = createApiClient(event),
): Promise<ServerLink[]> {
  const response = await client.get<LinkDeltaResponse>(
    "/v1/links?since=&limit=100",
    { auth: "required" },
  );
  return response.links.filter(
    (link) => link.status !== "archived" && link.status !== "deleted",
  );
}

export async function archiveLink(
  link_id: string,
  event?: RequestEventCommon,
  client: LinkApiClient = createApiClient(event),
): Promise<ServerLink> {
  const response = await client.patch<LinkMutationResponse>(
    `/v1/links/${encodeURIComponent(link_id)}`,
    { status: "archived" },
    { auth: "required" },
  );
  return response.link;
}

export async function deleteLink(
  link_id: string,
  event?: RequestEventCommon,
  client: LinkApiClient = createApiClient(event),
): Promise<boolean> {
  const response = await client.delete<DeleteLinkResponse>(
    `/v1/links/${encodeURIComponent(link_id)}`,
    { auth: "required" },
  );
  return response.success;
}
