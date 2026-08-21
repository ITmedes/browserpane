import { apiOrigin, fetchJson, poll } from './workflow-smoke-lib.mjs';

export class GatewayProtocolSmokeApi {
  #accessToken;
  #options;

  constructor(accessToken, options) {
    if (!accessToken) throw new Error('gateway protocol smoke requires an owner access token');
    this.#accessToken = accessToken;
    this.#options = options;
  }

  async createSession(purpose) {
    return await this.#request('/api/v1/sessions', {
      method: 'POST',
      headers: this.#jsonHeaders(),
      body: JSON.stringify({ labels: { suite: 'gateway-protocol', purpose } }),
    });
  }

  async issueTicket(sessionId) {
    return await this.#request(
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/access-tokens`,
      { method: 'POST', headers: this.#headers() },
    );
  }

  async getSession(sessionId) {
    return await this.#request(`/api/v1/sessions/${encodeURIComponent(sessionId)}`, {
      headers: this.#headers(),
    });
  }

  async waitForClients(sessionId, expectedClients) {
    return await poll(
      `session ${sessionId} client count ${expectedClients}`,
      async () => await this.getSession(sessionId),
      (session) => session.status?.connection_counts?.total_clients === expectedClients,
      this.#options.connectTimeoutMs,
      250,
    );
  }

  async kill(sessionId) {
    const response = await fetch(
      `${apiOrigin(this.#options)}/api/v1/sessions/${encodeURIComponent(sessionId)}/kill`,
      {
        method: 'POST',
        headers: this.#headers(),
        signal: AbortSignal.timeout(this.#options.connectTimeoutMs),
      },
    );
    if (!response.ok && response.status !== 404) {
      throw new Error(`failed to clean up protocol smoke session: HTTP ${response.status}`);
    }
  }

  async #request(path, init) {
    return await fetchJson(`${apiOrigin(this.#options)}${path}`, {
      ...init,
      signal: AbortSignal.timeout(this.#options.connectTimeoutMs),
    });
  }

  #headers() {
    return { Authorization: `Bearer ${this.#accessToken}` };
  }

  #jsonHeaders() {
    return { ...this.#headers(), 'Content-Type': 'application/json' };
  }
}
