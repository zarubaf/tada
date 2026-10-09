import type { Api, OrganizationFeature } from "../api/client";

/**
 * Loads the MCP switch of the organization (ADR 0045). The token page and the organization page
 * both read it. Resolves to nothing when the call fails; the caller shows its own failure.
 */
export async function loadMcpSwitch(api: Api): Promise<OrganizationFeature | undefined> {
  try {
    const { data } = await api.GET("/api/v1/organization/features");
    return data?.items.find((item) => item.feature === "mcp-tokens");
  } catch {
    return undefined;
  }
}
