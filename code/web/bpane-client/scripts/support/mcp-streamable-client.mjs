const DEFAULT_PROTOCOL_VERSION = '2025-11-25';
const JSON_RPC_VERSION = '2.0';

export class McpStreamableClient {
  #endpointUrl;
  #requestTimeoutMs;
  #sessionId = '';
  #nextRequestId = 1;
  #protocolVersion = DEFAULT_PROTOCOL_VERSION;

  constructor({ endpointUrl, requestTimeoutMs }) {
    this.#endpointUrl = endpointUrl;
    this.#requestTimeoutMs = requestTimeoutMs;
  }

  async initialize() {
    const result = await this.#request('initialize', {
      protocolVersion: DEFAULT_PROTOCOL_VERSION,
      capabilities: {},
      clientInfo: {
        name: 'bpane-multi-session-smoke',
        version: '0.1.0',
      },
    });
    if (typeof result?.protocolVersion === 'string') {
      this.#protocolVersion = result.protocolVersion;
    }
    await this.#notification('notifications/initialized', {});
    return result;
  }

  async listTools() {
    const result = await this.#request('tools/list', {});
    return Array.isArray(result?.tools) ? result.tools : [];
  }

  async callTool(name, args) {
    return await this.#request('tools/call', { name, arguments: args });
  }

  async close() {
    if (!this.#sessionId) {
      return;
    }
    const sessionId = this.#sessionId;
    this.#sessionId = '';
    const response = await fetch(this.#endpointUrl, {
      method: 'DELETE',
      headers: this.#headers({ sessionId }),
    });
    await response.body?.cancel();
    if (!response.ok && response.status !== 405 && response.status !== 404) {
      throw new Error(`MCP session close failed with HTTP ${response.status}`);
    }
  }

  async #request(method, params) {
    const id = this.#nextRequestId++;
    const response = await this.#post({ jsonrpc: JSON_RPC_VERSION, id, method, params });
    const message = await readJsonRpcResponse(response, id, this.#requestTimeoutMs);
    if (message.error) {
      throw new Error(`MCP ${method} failed: ${JSON.stringify(message.error)}`);
    }
    return message.result;
  }

  async #notification(method, params) {
    const response = await this.#post({ jsonrpc: JSON_RPC_VERSION, method, params });
    await response.body?.cancel();
  }

  async #post(message) {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.#requestTimeoutMs);
    try {
      const response = await fetch(this.#endpointUrl, {
        method: 'POST',
        headers: this.#headers(),
        body: JSON.stringify(message),
        signal: controller.signal,
      });
      const sessionId = response.headers.get('mcp-session-id');
      if (sessionId) {
        this.#sessionId = sessionId;
      }
      if (!response.ok) {
        const detail = await response.text().catch(() => '');
        throw new Error(`MCP HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
      }
      return response;
    } finally {
      clearTimeout(timer);
    }
  }

  #headers({ sessionId = this.#sessionId } = {}) {
    const headers = {
      accept: 'application/json, text/event-stream',
      'content-type': 'application/json',
    };
    if (sessionId) {
      headers['mcp-session-id'] = sessionId;
      headers['mcp-protocol-version'] = this.#protocolVersion;
    }
    return headers;
  }
}

async function readJsonRpcResponse(response, expectedId, timeoutMs) {
  const contentType = response.headers.get('content-type') ?? '';
  if (contentType.includes('application/json')) {
    return selectJsonRpcResponse(await response.json(), expectedId);
  }
  if (!contentType.includes('text/event-stream')) {
    await response.body?.cancel();
    throw new Error(`Unexpected MCP response content type: ${contentType || 'none'}`);
  }
  return await readSseJsonRpcResponse(response, expectedId, timeoutMs);
}

function selectJsonRpcResponse(payload, expectedId) {
  const messages = Array.isArray(payload) ? payload : [payload];
  const response = messages.find((message) => message?.id === expectedId);
  if (!response) {
    throw new Error(`MCP response ${expectedId} was not present in JSON payload`);
  }
  return response;
}

async function readSseJsonRpcResponse(response, expectedId, timeoutMs) {
  const reader = response.body?.getReader();
  if (!reader) {
    throw new Error('MCP response did not include a readable body');
  }

  const decoder = new TextDecoder();
  let buffer = '';
  const timer = setTimeout(() => {
    void reader.cancel(new Error(`Timed out waiting for MCP response ${expectedId}`));
  }, timeoutMs);

  try {
    while (true) {
      const { value, done } = await reader.read();
      if (done) {
        break;
      }
      buffer += decoder.decode(value, { stream: true });
      const events = buffer.split(/\r?\n\r?\n/);
      buffer = events.pop() ?? '';
      for (const event of events) {
        const message = parseSseJsonRpcMessage(event);
        if (message?.id === expectedId) {
          await reader.cancel();
          return message;
        }
      }
    }
  } finally {
    clearTimeout(timer);
  }

  throw new Error(`MCP response ${expectedId} was not present in SSE stream`);
}

function parseSseJsonRpcMessage(event) {
  const data = event
    .split(/\r?\n/)
    .filter((line) => line.startsWith('data:'))
    .map((line) => line.slice('data:'.length).trimStart())
    .join('\n');
  if (!data) {
    return null;
  }
  return JSON.parse(data);
}
