const GRAPH_BASE = "https://graph.microsoft.com/v1.0";
const TOKEN_URL_BASE = "https://login.microsoftonline.com";

interface TokenCache {
  accessToken: string;
  expiresAt: number;
}

export class MsGraphClient {
  private readonly clientId: string;
  private readonly clientSecret: string;
  private readonly tenantId: string;
  private tokenCache: TokenCache | null = null;

  constructor() {
    const clientId = process.env["MS_TEAMS_CLIENT_ID"] ?? process.env["MS_GRAPH_CLIENT_ID"];
    const clientSecret = process.env["MS_TEAMS_CLIENT_SECRET"] ?? process.env["MS_GRAPH_CLIENT_SECRET"];
    const tenantId = process.env["MS_TEAMS_TENANT_ID"] ?? process.env["MS_GRAPH_TENANT_ID"];

    if (!clientId || !clientSecret || !tenantId) {
      const missing: string[] = [];
      if (!clientId) missing.push("MS_TEAMS_CLIENT_ID (or MS_GRAPH_CLIENT_ID)");
      if (!clientSecret) missing.push("MS_TEAMS_CLIENT_SECRET (or MS_GRAPH_CLIENT_SECRET)");
      if (!tenantId) missing.push("MS_TEAMS_TENANT_ID (or MS_GRAPH_TENANT_ID)");
      process.stderr.write(
        `teams-cli: not configured — missing env vars: ${missing.join(", ")}\n`
      );
      process.exit(1);
    }

    this.clientId = clientId;
    this.clientSecret = clientSecret;
    this.tenantId = tenantId;
  }

  private async fetchToken(): Promise<string> {
    const now = Date.now();
    if (this.tokenCache && this.tokenCache.expiresAt > now + 60_000) {
      return this.tokenCache.accessToken;
    }

    const url = `${TOKEN_URL_BASE}/${this.tenantId}/oauth2/v2.0/token`;
    const body = new URLSearchParams({
      grant_type: "client_credentials",
      client_id: this.clientId,
      client_secret: this.clientSecret,
      scope: "https://graph.microsoft.com/.default",
    });

    const resp = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: body.toString(),
    });

    if (!resp.ok) {
      const text = await resp.text();
      process.stderr.write(`teams-cli: token fetch failed (${resp.status}): ${text}\n`);
      process.exit(1);
    }

    const data = (await resp.json()) as { access_token: string; expires_in: number };
    this.tokenCache = {
      accessToken: data.access_token,
      expiresAt: now + data.expires_in * 1000,
    };
    return this.tokenCache.accessToken;
  }

  async get(path: string): Promise<unknown> {
    const token = await this.fetchToken();
    const url = path.startsWith("http") ? path : `${GRAPH_BASE}${path}`;
    const resp = await fetch(url, {
      headers: { Authorization: `Bearer ${token}` },
    });
    return this.handleResponse(resp, path);
  }

  async post(path: string, body: unknown): Promise<unknown> {
    const token = await this.fetchToken();
    const url = path.startsWith("http") ? path : `${GRAPH_BASE}${path}`;
    const resp = await fetch(url, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(body),
    });
    return this.handleResponse(resp, path);
  }

  private async handleResponse(resp: Response, resource: string): Promise<unknown> {
    if (resp.status === 403) {
      const data = (await resp.json().catch(() => ({}))) as {
        error?: { message?: string };
      };
      const msg = data.error?.message ?? "insufficient permissions";
      process.stderr.write(
        `teams-cli: 403 Forbidden — ${msg}\n` +
          `  Required permissions: Chat.Read.All, ChannelMessage.Read.All, ChatMessage.Send, Presence.Read.All\n`
      );
      process.exit(1);
    }

    if (resp.status === 404) {
      const id = resource.split("/").pop() ?? resource;
      process.stderr.write(`teams-cli: Not found: ${id}\n`);
      process.exit(1);
    }

    if (!resp.ok) {
      const text = await resp.text();
      process.stderr.write(`teams-cli: API error (${resp.status}): ${text}\n`);
      process.exit(1);
    }

    if (resp.status === 204) {
      return null;
    }

    return resp.json();
  }
}
