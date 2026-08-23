import {
  ComposeResourceRegistry,
  namespaceApiPayload,
  resourceRecords,
} from './resource-registry.mjs';

const MUTATING_METHODS = new Set(['POST', 'PUT', 'PATCH']);

export function installNamespacedFetch(environment = process.env) {
  const namespace = environment.BPANE_CI_STAGE_NAMESPACE;
  const registryPath = environment.BPANE_CI_RESOURCE_REGISTRY;
  if (!namespace || !registryPath || globalThis.fetch?.__bpaneNamespaced === true) return;
  const registry = new ComposeResourceRegistry(registryPath, namespace);
  const originalFetch = globalThis.fetch.bind(globalThis);
  const namespacedFetch = async (input, init = {}) => {
    const url = requestUrl(input);
    const method = String(init.method ?? input?.method ?? 'GET').toUpperCase();
    const response = await originalFetch(input, rewriteJsonRequest(url, method, init, namespace));
    if (response.ok && MUTATING_METHODS.has(method)) {
      const body = await response.clone().json().catch(() => null);
      recordResources(registry, url, body);
    }
    return response;
  };
  Object.defineProperty(namespacedFetch, '__bpaneNamespaced', { value: true });
  globalThis.fetch = namespacedFetch;
}

export async function installNamespacedPlaywrightTarget(target, environment = process.env) {
  const namespace = environment.BPANE_CI_STAGE_NAMESPACE;
  const registryPath = environment.BPANE_CI_RESOURCE_REGISTRY;
  if (!namespace || !registryPath || typeof target?.route !== 'function') return;
  const registry = new ComposeResourceRegistry(registryPath, namespace);
  await target.route('**/api/v1/**', async (route) => {
    try {
      const request = route.request();
      const method = request.method().toUpperCase();
      if (!MUTATING_METHODS.has(method)) {
        await route.continue();
        return;
      }
      const url = request.url();
      const response = await route.fetch(namespacedRequestOverrides(
        request, url, method, namespace,
      ));
      const body = await response.body();
      if (response.ok()) {
        recordResources(registry, url, parseJsonBody(body));
      }
      await route.fulfill({ response, body });
    } catch (error) {
      if (!isTargetClosedError(error)) throw error;
    }
  });
}

function rewriteJsonRequest(url, method, init, namespace) {
  if (typeof init.body !== 'string') return init;
  try {
    return { ...init, body: JSON.stringify(namespaceApiPayload(
      url, method, JSON.parse(init.body), namespace,
    )) };
  } catch {
    return init;
  }
}

function namespacedPostData(url, method, postData, namespace) {
  if (!postData) return postData;
  try {
    return JSON.stringify(namespaceApiPayload(url, method, JSON.parse(postData), namespace));
  } catch {
    return postData;
  }
}

function namespacedRequestOverrides(request, url, method, namespace) {
  const postData = request.postData();
  const rewrittenPostData = namespacedPostData(url, method, postData, namespace);
  if (rewrittenPostData !== postData) return { postData: rewrittenPostData };
  if (!isBrowserContextImport(url)) return {};
  const headers = request.headers();
  let labels = {};
  try {
    labels = JSON.parse(headers['x-bpane-browser-context-labels'] ?? '{}');
  } catch {
    labels = {};
  }
  return {
    headers: {
      ...headers,
      'x-bpane-browser-context-labels': JSON.stringify({
        ...labels,
        bpane_ci_namespace: namespace,
      }),
    },
  };
}

function isBrowserContextImport(url) {
  try {
    return new URL(url, 'http://localhost').pathname === '/api/v1/browser-contexts/import';
  } catch {
    return false;
  }
}

function recordResources(registry, url, body) {
  for (const record of resourceRecords(url, body)) registry.record(record.kind, record.id);
}

function parseJsonBody(body) {
  try {
    return JSON.parse(body.toString('utf8'));
  } catch {
    return null;
  }
}

function isTargetClosedError(error) {
  const name = error instanceof Error ? error.name : '';
  const message = error instanceof Error ? error.message : '';
  return name === 'TargetClosedError'
    || message.includes('Target page, context or browser has been closed');
}

function requestUrl(input) {
  if (typeof input === 'string') return input;
  if (input instanceof URL) return input.toString();
  return typeof input?.url === 'string' ? input.url : '';
}
