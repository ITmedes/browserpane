export type AuthExampleUser = {
  readonly username: string;
  readonly password: string;
};

export type McpBridgeConfig = {
  readonly controlUrl: string;
  readonly clientId: string;
  readonly issuer: string;
  readonly displayName: string;
};

export type AuthConfig = {
  readonly mode: string;
  readonly providerHint?: string;
  readonly issuer?: string;
  readonly clientId?: string;
  readonly scope?: string;
  readonly exampleUser?: AuthExampleUser;
  readonly mcpBridge?: McpBridgeConfig;
};

export type AuthConfigClientOptions = {
  readonly baseUrl: string | URL;
  readonly fetchImpl?: typeof fetch;
};

export class AuthConfigClient {
  readonly #baseUrl: URL;
  readonly #fetchImpl: typeof fetch;

  constructor(options: AuthConfigClientOptions) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#fetchImpl = options.fetchImpl ?? fetch;
  }

  async load(): Promise<AuthConfig | null> {
    const response = await this.#fetchImpl(new URL('/auth-config.json', this.#baseUrl));
    if (response.status === 404) {
      return null;
    }
    if (!response.ok) {
      throw new Error(`auth config request failed with HTTP ${response.status}`);
    }
    return AuthConfigMapper.toAuthConfig(await response.json());
  }
}

export class AuthConfigMapper {
  static toAuthConfig(payload: unknown): AuthConfig {
    const object = expectRecord(payload, 'auth config');
    const mode = expectString(object.mode, 'auth config mode');
    const providerHint = optionalString(object.providerHint, 'auth config providerHint');
    const issuer = optionalString(object.issuer, 'auth config issuer');
    const clientId = optionalString(object.clientId, 'auth config clientId');
    const scope = optionalString(object.scope, 'auth config scope');
    const exampleUser = optionalExampleUser(object.exampleUser);
    const mcpBridge = optionalMcpBridgeConfig(object.mcpBridge);
    return {
      mode,
      ...(providerHint !== undefined ? { providerHint } : {}),
      ...(issuer !== undefined ? { issuer } : {}),
      ...(clientId !== undefined ? { clientId } : {}),
      ...(scope !== undefined ? { scope } : {}),
      ...(exampleUser !== undefined ? { exampleUser } : {}),
      ...(mcpBridge !== undefined ? { mcpBridge } : {}),
    };
  }
}

function optionalExampleUser(value: unknown): AuthExampleUser | undefined {
  if (value === undefined || value === null) {
    return undefined;
  }
  const object = expectRecord(value, 'auth config exampleUser');
  return {
    username: expectString(object.username, 'auth config exampleUser username'),
    password: expectString(object.password, 'auth config exampleUser password'),
  };
}

function optionalMcpBridgeConfig(value: unknown): McpBridgeConfig | undefined {
  if (value === undefined || value === null) {
    return undefined;
  }
  const object = expectRecord(value, 'auth config mcpBridge');
  return {
    controlUrl: expectString(object.controlUrl, 'auth config mcpBridge controlUrl'),
    clientId: expectString(object.clientId, 'auth config mcpBridge clientId'),
    issuer: expectString(object.issuer, 'auth config mcpBridge issuer'),
    displayName: expectString(object.displayName, 'auth config mcpBridge displayName'),
  };
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  return value as Record<string, unknown>;
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`${label} must be a non-empty string`);
  }
  return value;
}

function optionalString(value: unknown, label: string): string | undefined {
  if (value === undefined || value === null || value === '') {
    return undefined;
  }
  return expectString(value, label);
}
