import type { BrowserSessionConnectOptions, BrowserSessionHandle, BrowserSessionSdk } from './browser-session-types';

const DEFAULT_SDK_URL = '/dist/bpane.js';
const DEFAULT_SDK_CACHE_PARAM = 'bpane_admin_sdk';

export type BrowserSessionModuleImporter = (moduleUrl: string) => Promise<unknown>;

export type BrowserSessionSdkLoaderOptions = {
  readonly moduleUrl?: string;
  readonly importer?: BrowserSessionModuleImporter;
  readonly cacheBust?: boolean;
  readonly cacheToken?: string;
};

export class BrowserSessionSdkLoader {
  readonly #moduleUrl: string;
  readonly #importer: BrowserSessionModuleImporter;

  constructor(options: BrowserSessionSdkLoaderOptions = {}) {
    const cacheBust = options.cacheBust ?? !options.moduleUrl;
    this.#moduleUrl = cacheBust
      ? appendCacheToken(options.moduleUrl ?? DEFAULT_SDK_URL, options.cacheToken ?? defaultCacheToken())
      : options.moduleUrl ?? DEFAULT_SDK_URL;
    this.#importer = options.importer ?? defaultBrowserSessionModuleImporter;
  }

  async load(): Promise<BrowserSessionSdk> {
    return BrowserSessionSdkMapper.toSdk(await this.#importer(this.#moduleUrl));
  }
}

class BrowserSessionSdkMapper {
  static toSdk(moduleValue: unknown): BrowserSessionSdk {
    const moduleObject = expectRecord(moduleValue, 'BrowserPane SDK module');
    const sessionClass = expectObjectLike(moduleObject.BpaneSession, 'BrowserPane BpaneSession export');
    const connect = sessionClass.connect;
    if (typeof connect !== 'function') {
      throw new Error('BrowserPane SDK BpaneSession.connect must be a function');
    }
    return {
      BpaneSession: {
        connect: async (options) => await invokeConnect(connect, options),
      },
    };
  }
}

async function defaultBrowserSessionModuleImporter(moduleUrl: string): Promise<unknown> {
  return await import(/* @vite-ignore */ moduleUrl);
}

async function invokeConnect(
  connect: Function,
  options: BrowserSessionConnectOptions,
): Promise<BrowserSessionHandle> {
  const handle = await connect(options);
  const object = expectRecord(handle, 'BrowserPane session handle');
  if (typeof object.disconnect !== 'function') {
    throw new Error('BrowserPane session handle must expose disconnect');
  }
  return object as BrowserSessionHandle;
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  return value as Record<string, unknown>;
}

function expectObjectLike(value: unknown, label: string): Record<string, unknown> {
  if ((!value || typeof value !== 'object') && typeof value !== 'function') {
    throw new Error(`${label} must be an object or function`);
  }
  return value as Record<string, unknown>;
}

function appendCacheToken(moduleUrl: string, cacheToken: string): string {
  const hashIndex = moduleUrl.indexOf('#');
  const baseUrl = hashIndex === -1 ? moduleUrl : moduleUrl.slice(0, hashIndex);
  const hash = hashIndex === -1 ? '' : moduleUrl.slice(hashIndex);
  const separator = baseUrl.includes('?') ? '&' : '?';
  return `${baseUrl}${separator}${DEFAULT_SDK_CACHE_PARAM}=${encodeURIComponent(cacheToken)}${hash}`;
}

function defaultCacheToken(): string {
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
}
