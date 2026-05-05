import type { BrowserSessionConnectOptions, BrowserSessionHandle, BrowserSessionSdk } from './browser-session-types';

const DEFAULT_SDK_URL = '/dist/bpane.js';

export type BrowserSessionModuleImporter = (moduleUrl: string) => Promise<unknown>;

export type BrowserSessionSdkLoaderOptions = {
  readonly moduleUrl?: string;
  readonly importer?: BrowserSessionModuleImporter;
};

export class BrowserSessionSdkLoader {
  readonly #moduleUrl: string;
  readonly #importer: BrowserSessionModuleImporter;

  constructor(options: BrowserSessionSdkLoaderOptions = {}) {
    this.#moduleUrl = options.moduleUrl ?? DEFAULT_SDK_URL;
    this.#importer = options.importer ?? defaultBrowserSessionModuleImporter;
  }

  async load(): Promise<BrowserSessionSdk> {
    return BrowserSessionSdkMapper.toSdk(await this.#importer(this.#moduleUrl));
  }
}

class BrowserSessionSdkMapper {
  static toSdk(moduleValue: unknown): BrowserSessionSdk {
    const moduleObject = expectRecord(moduleValue, 'BrowserPane SDK module');
    const sessionClass = expectRecord(moduleObject.BpaneSession, 'BrowserPane BpaneSession export');
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
