type GatewayTokenManagerOptions = {
  staticAutomationAccessToken: string;
  staticBearerToken: string;
  tokenUrl: string;
  clientId: string;
  clientSecret: string;
  scopes: string;
};

export class GatewayTokenManager {
  private readonly staticAutomationAccessToken: string;
  private readonly staticBearerToken: string;
  private readonly tokenUrl: string;
  private readonly clientId: string;
  private readonly clientSecret: string;
  private readonly scopes: string;
  private accessToken: string | null = null;
  private expiresAtMs = 0;

  constructor(options: GatewayTokenManagerOptions) {
    this.staticAutomationAccessToken = options.staticAutomationAccessToken.trim();
    this.staticBearerToken = options.staticBearerToken.trim();
    this.tokenUrl = options.tokenUrl.trim();
    this.clientId = options.clientId.trim();
    this.clientSecret = options.clientSecret.trim();
    this.scopes = options.scopes.trim();
  }

  async getAuthHeaders(
    extraHeaders: Record<string, string> = {},
  ): Promise<Record<string, string>> {
    if (this.staticAutomationAccessToken) {
      return {
        ...extraHeaders,
        "x-bpane-automation-access-token": this.staticAutomationAccessToken,
      };
    }
    const token = await this.getAccessToken();
    if (!token) {
      return extraHeaders;
    }
    return {
      ...extraHeaders,
      Authorization: `Bearer ${token}`,
    };
  }

  private async getAccessToken(): Promise<string | null> {
    if (this.staticBearerToken) {
      return this.staticBearerToken;
    }
    if (!this.tokenUrl || !this.clientId || !this.clientSecret) {
      return null;
    }

    const now = Date.now();
    if (this.accessToken && now < this.expiresAtMs - 30_000) {
      return this.accessToken;
    }

    const body = new URLSearchParams({
      grant_type: "client_credentials",
      client_id: this.clientId,
      client_secret: this.clientSecret,
    });
    if (this.scopes) {
      body.set("scope", this.scopes);
    }

    const response = await fetch(this.tokenUrl, {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body,
    });
    if (!response.ok) {
      throw new Error(`token endpoint returned ${response.status}`);
    }

    const payload = (await response.json()) as {
      access_token?: string;
      expires_in?: number;
    };
    if (!payload.access_token) {
      throw new Error("token endpoint returned no access_token");
    }

    this.accessToken = payload.access_token;
    this.expiresAtMs = now + Math.max(30, payload.expires_in ?? 60) * 1000;
    return this.accessToken;
  }
}
