import { createHmac } from "node:crypto";
import { lookup } from "node:dns/promises";
import { promises as fs } from "node:fs";
import net from "node:net";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { chromium, type Browser, type BrowserContext, type Page } from "playwright-core";
import type {
  GatewayWorkflowRunProducedFileResource,
  WorkflowResolvedCredentialBinding,
  WorkflowRunnerContext,
  WorkflowRunnerCredentialBinding,
  WorkflowRunnerWorkspaceInput,
} from "./types.js";

type WorkflowExecutionContext = {
  browser: Browser;
  context: BrowserContext;
  page: Page;
  artifacts: {
    uploadFile: (request: {
      workspaceId: string;
      fileName: string;
      bytes: Uint8Array | Buffer;
      mediaType?: string | null;
      provenance?: Record<string, unknown> | null;
    }) => Promise<GatewayWorkflowRunProducedFileResource>;
    uploadTextFile: (request: {
      workspaceId: string;
      fileName: string;
      text: string;
      mediaType?: string | null;
      provenance?: Record<string, unknown> | null;
    }) => Promise<GatewayWorkflowRunProducedFileResource>;
  };
  credentialBindings: WorkflowRunnerCredentialBinding[];
  credentials: {
    bindings: WorkflowRunnerCredentialBinding[];
    load: (bindingId: string, targetOrigin: string) => Promise<WorkflowResolvedCredentialBinding>;
    apply: (
      bindingId: string,
      request?: {
        page?: Page;
        targetOrigin?: string;
      },
    ) => Promise<unknown>;
    generateTotp: (
      bindingId: string,
      targetOrigin: string,
    ) => Promise<{
      code: string;
      digits: number;
      periodSec: number;
      generatedAt: number;
      expiresAt: number;
    }>;
  };
  input: unknown;
  workspaceInputs: WorkflowRunnerWorkspaceInput[];
  sessionId: string;
  workflowRunId: string;
  automationTaskId: string;
  sourceRoot: string;
};

type WorkflowEntrypointModule = {
  default?: (context: WorkflowExecutionContext) => Promise<unknown> | unknown;
  run?: (context: WorkflowExecutionContext) => Promise<unknown> | unknown;
};

async function main(): Promise<void> {
  const contextPath = process.argv[2];
  if (!contextPath) {
    throw new Error("workflow runner requires a context file path argument");
  }
  const rawContext = await fs.readFile(contextPath, "utf8");
  const context = JSON.parse(rawContext) as WorkflowRunnerContext;
  const browser = await chromium.connectOverCDP(
    await normalizeCdpEndpointUrl(context.endpointUrl),
    {
      headers: {
        [context.authHeader]: context.authToken,
      },
    },
  );
  try {
    const page = await createExecutionPage(browser);
    const module = (await import(pathToFileURL(context.entrypointPath).href)) as WorkflowEntrypointModule;
    const execute = resolveEntrypointFunction(module, context.entrypointPath);
    const output = await execute({
      browser,
      context: page.context(),
      page,
      artifacts: createWorkflowArtifactApi(context),
      credentialBindings: context.credentialBindings,
      credentials: {
        bindings: context.credentialBindings,
        load: async (bindingId: string, targetOrigin: string) =>
          loadResolvedCredentialBinding(context, bindingId, targetOrigin),
        apply: async (
          bindingId: string,
          request: { page?: Page; targetOrigin?: string } = {},
        ) =>
          applyCredentialBinding(context, {
            bindingId,
            page: request.page ?? page,
            targetOrigin: request.targetOrigin,
          }),
        generateTotp: async (bindingId: string, targetOrigin: string) =>
          generateCredentialTotp(context, bindingId, targetOrigin),
      },
      input: context.input,
      workspaceInputs: context.workspaceInputs,
      sessionId: context.sessionId,
      workflowRunId: context.workflowRunId,
      automationTaskId: context.automationTaskId,
      sourceRoot: context.sourceRoot,
    });
    await fs.mkdir(path.dirname(context.resultPath), { recursive: true });
    await fs.writeFile(
      context.resultPath,
      `${JSON.stringify({ output: ensureJsonSerializable(output) }, null, 2)}\n`,
      "utf8",
    );
  } finally {
    await browser.close().catch(() => {});
  }
}

async function normalizeCdpEndpointUrl(endpointUrl: string): Promise<string> {
  const url = new URL(endpointUrl);
  if (url.hostname === "localhost" || net.isIP(url.hostname)) {
    return url.toString();
  }
  const resolved = await lookup(url.hostname);
  url.hostname = resolved.address;
  return url.toString();
}

async function createExecutionPage(browser: Browser): Promise<Page> {
  const context = browser.contexts()[0];
  if (!context) {
    throw new Error("workflow session did not expose a browser context over CDP");
  }
  const page = await context.newPage();
  await page.bringToFront().catch(() => {});
  return page;
}

function resolveEntrypointFunction(
  module: WorkflowEntrypointModule,
  entrypointPath: string,
): (context: WorkflowExecutionContext) => Promise<unknown> | unknown {
  if (typeof module.default === "function") {
    return module.default;
  }
  if (typeof module.run === "function") {
    return module.run;
  }
  throw new Error(
    `workflow entrypoint ${entrypointPath} must export a default function or named run()`,
  );
}

async function loadResolvedCredentialBinding(
  context: WorkflowRunnerContext,
  bindingId: string,
  targetOrigin: string,
): Promise<WorkflowResolvedCredentialBinding> {
  const binding = context.credentialBindings.find((entry) => entry.id === bindingId);
  if (!binding) {
    throw new Error(`unknown workflow credential binding ${bindingId}`);
  }
  const normalizedTargetOrigin = normalizeOrigin(targetOrigin);
  if (binding.allowedOrigins.length > 0) {
    const allowedOrigins = binding.allowedOrigins.map(normalizeOrigin);
    if (!allowedOrigins.includes(normalizedTargetOrigin)) {
      throw new Error(
        `workflow credential binding ${bindingId} is not allowed for origin ${normalizedTargetOrigin}`,
      );
    }
  }
  const localPath = context.credentialBindingFiles[bindingId];
  if (!localPath) {
    throw new Error(`workflow credential binding ${bindingId} is missing local materialization`);
  }
  const payload = JSON.parse(await fs.readFile(localPath, "utf8")) as { payload?: unknown };
  return {
    ...binding,
    payload: payload.payload ?? null,
  };
}

async function applyCredentialBinding(
  context: WorkflowRunnerContext,
  request: {
    bindingId: string;
    page: Page;
    targetOrigin?: string;
  },
): Promise<unknown> {
  const targetOrigin = resolveCredentialTargetOrigin(request.page, request.targetOrigin);
  const binding = await loadResolvedCredentialBinding(context, request.bindingId, targetOrigin);
  switch (binding.injectionMode) {
    case "cookie_seed": {
      const cookies = parseCookieSeedPayload(binding.payload, targetOrigin);
      await request.page.context().addCookies(cookies);
      return {
        mode: binding.injectionMode,
        cookie_count: cookies.length,
      };
    }
    case "storage_seed": {
      const storage = parseStorageSeedPayload(binding.payload);
      await seedStorageForOrigin(request.page, targetOrigin, storage);
      return {
        mode: binding.injectionMode,
        local_storage_count: Object.keys(storage.localStorageEntries).length,
        session_storage_count: Object.keys(storage.sessionStorageEntries).length,
      };
    }
    case "form_fill": {
      const fields = parseFormFillPayload(binding.payload);
      for (const field of fields) {
        await request.page.locator(field.selector).fill(field.value);
      }
      return {
        mode: binding.injectionMode,
        field_count: fields.length,
      };
    }
    case "totp_fill": {
      const payload = parseTotpPayload(binding.payload);
      const generated = generateTotpCode({
        secret: payload.secret,
        digits: binding.totp?.digits ?? payload.digits ?? 6,
        periodSec: binding.totp?.period_sec ?? payload.periodSec ?? 30,
      });
      await request.page.locator(payload.selector).fill(generated.code);
      return {
        mode: binding.injectionMode,
        ...generated,
      };
    }
  }
}

async function generateCredentialTotp(
  context: WorkflowRunnerContext,
  bindingId: string,
  targetOrigin: string,
): Promise<{
  code: string;
  digits: number;
  periodSec: number;
  generatedAt: number;
  expiresAt: number;
}> {
  const binding = await loadResolvedCredentialBinding(context, bindingId, targetOrigin);
  if (binding.injectionMode !== "totp_fill") {
    throw new Error(`workflow credential binding ${bindingId} is not configured for TOTP fill`);
  }
  const payload = parseTotpPayload(binding.payload);
  return generateTotpCode({
    secret: payload.secret,
    digits: binding.totp?.digits ?? payload.digits ?? 6,
    periodSec: binding.totp?.period_sec ?? payload.periodSec ?? 30,
  });
}

function normalizeOrigin(value: string): string {
  return new URL(value).origin;
}

function resolveCredentialTargetOrigin(page: Page, explicitTargetOrigin?: string): string {
  if (explicitTargetOrigin) {
    return normalizeOrigin(explicitTargetOrigin);
  }
  const currentUrl = page.url();
  if (!currentUrl || currentUrl === "about:blank") {
    throw new Error(
      "workflow credential injection requires targetOrigin when the current page has no origin",
    );
  }
  return normalizeOrigin(currentUrl);
}

function ensureObject(
  value: unknown,
  label: string,
): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be a JSON object`);
  }
  return value as Record<string, unknown>;
}

function parseCookieSeedPayload(
  value: unknown,
  targetOrigin: string,
): Array<{
  name: string;
  value: string;
  url?: string;
  domain?: string;
  path?: string;
  httpOnly?: boolean;
  secure?: boolean;
  sameSite?: "Strict" | "Lax" | "None";
  expires?: number;
}> {
  const payload = ensureObject(value, "workflow cookie seed payload");
  const cookies = payload.cookies;
  if (!Array.isArray(cookies) || cookies.length === 0) {
    throw new Error("workflow cookie seed payload must contain a non-empty cookies array");
  }
  return cookies.map((entry, index) => {
    const cookie = ensureObject(entry, `workflow cookie seed payload.cookies[${index}]`);
    const name = String(cookie.name ?? "").trim();
    const cookieValue = String(cookie.value ?? "");
    if (!name) {
      throw new Error(`workflow cookie seed payload.cookies[${index}] is missing name`);
    }
    const parsed = {
      name,
      value: cookieValue,
    };
    if (typeof cookie.domain === "string" && cookie.domain.trim()) {
      return {
        ...parsed,
        domain: cookie.domain.trim(),
        path: typeof cookie.path === "string" && cookie.path.trim() ? cookie.path.trim() : "/",
        httpOnly: cookie.httpOnly === true,
        secure: cookie.secure === true,
        sameSite: parseCookieSameSite(cookie.sameSite),
        expires: typeof cookie.expires === "number" ? cookie.expires : undefined,
      };
    }
    return {
      ...parsed,
      url:
        typeof cookie.url === "string" && cookie.url.trim()
          ? cookie.url.trim()
          : `${targetOrigin}/`,
      httpOnly: cookie.httpOnly === true,
      secure: cookie.secure === true,
      sameSite: parseCookieSameSite(cookie.sameSite),
      expires: typeof cookie.expires === "number" ? cookie.expires : undefined,
    };
  });
}

function parseCookieSameSite(value: unknown): "Strict" | "Lax" | "None" | undefined {
  if (typeof value !== "string" || !value.trim()) {
    return undefined;
  }
  const normalized = value.trim().toLowerCase();
  switch (normalized) {
    case "strict":
      return "Strict";
    case "lax":
      return "Lax";
    case "none":
      return "None";
    default:
      throw new Error(`workflow cookie sameSite value ${value} is not supported`);
  }
}

function parseStorageSeedPayload(value: unknown): {
  localStorageEntries: Record<string, string>;
  sessionStorageEntries: Record<string, string>;
} {
  const payload = ensureObject(value, "workflow storage seed payload");
  const localStorageEntries = parseStringRecord(payload.local_storage, "local_storage");
  const sessionStorageEntries = parseStringRecord(payload.session_storage, "session_storage");
  if (
    Object.keys(localStorageEntries).length === 0 &&
    Object.keys(sessionStorageEntries).length === 0
  ) {
    throw new Error(
      "workflow storage seed payload must define local_storage or session_storage entries",
    );
  }
  return {
    localStorageEntries,
    sessionStorageEntries,
  };
}

function parseStringRecord(value: unknown, label: string): Record<string, string> {
  if (value === null || typeof value === "undefined") {
    return {};
  }
  const payload = ensureObject(value, `workflow ${label}`);
  const entries: Record<string, string> = {};
  for (const [key, entry] of Object.entries(payload)) {
    const trimmedKey = key.trim();
    if (!trimmedKey) {
      throw new Error(`workflow ${label} contains an empty key`);
    }
    entries[trimmedKey] = String(entry ?? "");
  }
  return entries;
}

function parseFormFillPayload(
  value: unknown,
): Array<{
  selector: string;
  value: string;
}> {
  const payload = ensureObject(value, "workflow form fill payload");
  if (!Array.isArray(payload.fields) || payload.fields.length === 0) {
    throw new Error("workflow form fill payload must contain a non-empty fields array");
  }
  return payload.fields.map((entry, index) => {
    const field = ensureObject(entry, `workflow form fill payload.fields[${index}]`);
    const selector = String(field.selector ?? "").trim();
    if (!selector) {
      throw new Error(`workflow form fill payload.fields[${index}] is missing selector`);
    }
    return {
      selector,
      value: String(field.value ?? ""),
    };
  });
}

function parseTotpPayload(value: unknown): {
  secret: string;
  selector: string;
  digits: number | null;
  periodSec: number | null;
} {
  const payload = ensureObject(value, "workflow TOTP payload");
  const secret = String(payload.secret ?? payload.secret_base32 ?? "").trim();
  if (!secret) {
    throw new Error("workflow TOTP payload is missing secret");
  }
  const selector = String(payload.selector ?? payload.input_selector ?? "").trim();
  if (!selector) {
    throw new Error("workflow TOTP payload is missing selector");
  }
  return {
    secret,
    selector,
    digits:
      typeof payload.digits === "number" && Number.isFinite(payload.digits)
        ? Math.trunc(payload.digits)
        : null,
    periodSec:
      typeof payload.period_sec === "number" && Number.isFinite(payload.period_sec)
        ? Math.trunc(payload.period_sec)
        : null,
  };
}

async function seedStorageForOrigin(
  page: Page,
  targetOrigin: string,
  storage: {
    localStorageEntries: Record<string, string>;
    sessionStorageEntries: Record<string, string>;
  },
): Promise<void> {
  const seedPayload = {
    origin: targetOrigin,
    localStorageEntries: storage.localStorageEntries,
    sessionStorageEntries: storage.sessionStorageEntries,
  };
  await page.context().addInitScript(
    ({
      origin,
      localStorageEntries,
      sessionStorageEntries,
    }: {
      origin: string;
      localStorageEntries: Record<string, string>;
      sessionStorageEntries: Record<string, string>;
    }) => {
      if (window.location.origin !== origin) {
        return;
      }
      for (const [key, value] of Object.entries(localStorageEntries)) {
        window.localStorage.setItem(key, value);
      }
      for (const [key, value] of Object.entries(sessionStorageEntries)) {
        window.sessionStorage.setItem(key, value);
      }
    },
    seedPayload,
  );
  if (page.url() !== "about:blank" && normalizeOrigin(page.url()) === targetOrigin) {
    await page.evaluate(
      ({
        localStorageEntries,
        sessionStorageEntries,
      }: {
        localStorageEntries: Record<string, string>;
        sessionStorageEntries: Record<string, string>;
      }) => {
        for (const [key, value] of Object.entries(localStorageEntries)) {
          window.localStorage.setItem(key, value);
        }
        for (const [key, value] of Object.entries(sessionStorageEntries)) {
          window.sessionStorage.setItem(key, value);
        }
      },
      seedPayload,
    );
  }
}

function generateTotpCode(request: {
  secret: string;
  digits: number;
  periodSec: number;
}): {
  code: string;
  digits: number;
  periodSec: number;
  generatedAt: number;
  expiresAt: number;
} {
  const digits = request.digits > 0 ? request.digits : 6;
  const periodSec = request.periodSec > 0 ? request.periodSec : 30;
  const generatedAt = Date.now();
  const counter = Math.floor(generatedAt / 1000 / periodSec);
  const counterBuffer = Buffer.alloc(8);
  counterBuffer.writeBigUInt64BE(BigInt(counter));
  const secretBytes = decodeBase32Secret(request.secret);
  const digest = createHmac("sha1", secretBytes).update(counterBuffer).digest();
  const offset = digest[digest.length - 1] & 0x0f;
  const truncated =
    ((digest[offset] & 0x7f) << 24) |
    ((digest[offset + 1] & 0xff) << 16) |
    ((digest[offset + 2] & 0xff) << 8) |
    (digest[offset + 3] & 0xff);
  const code = String(truncated % 10 ** digits).padStart(digits, "0");
  return {
    code,
    digits,
    periodSec,
    generatedAt,
    expiresAt: (counter + 1) * periodSec * 1000,
  };
}

function decodeBase32Secret(value: string): Buffer {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
  const normalized = value.toUpperCase().replace(/=+$/u, "").replace(/\s+/gu, "");
  let bits = "";
  for (const char of normalized) {
    const index = alphabet.indexOf(char);
    if (index === -1) {
      throw new Error(`workflow TOTP secret contains invalid base32 character ${char}`);
    }
    bits += index.toString(2).padStart(5, "0");
  }
  const bytes: number[] = [];
  for (let index = 0; index + 8 <= bits.length; index += 8) {
    bytes.push(Number.parseInt(bits.slice(index, index + 8), 2));
  }
  return Buffer.from(bytes);
}

function createWorkflowArtifactApi(context: WorkflowRunnerContext) {
  return {
    uploadFile: async (request: {
      workspaceId: string;
      fileName: string;
      bytes: Uint8Array | Buffer;
      mediaType?: string | null;
      provenance?: Record<string, unknown> | null;
    }) =>
      uploadWorkflowProducedFile(context, {
        workspaceId: request.workspaceId,
        fileName: request.fileName,
        bytes: request.bytes instanceof Uint8Array ? request.bytes : new Uint8Array(request.bytes),
        mediaType: request.mediaType,
        provenance: request.provenance,
      }),
    uploadTextFile: async (request: {
      workspaceId: string;
      fileName: string;
      text: string;
      mediaType?: string | null;
      provenance?: Record<string, unknown> | null;
    }) =>
      uploadWorkflowProducedFile(context, {
        workspaceId: request.workspaceId,
        fileName: request.fileName,
        bytes: new TextEncoder().encode(request.text),
        mediaType: request.mediaType ?? "text/plain; charset=utf-8",
        provenance: request.provenance,
      }),
  };
}

async function uploadWorkflowProducedFile(
  context: WorkflowRunnerContext,
  request: {
    workspaceId: string;
    fileName: string;
    bytes: Uint8Array;
    mediaType?: string | null;
    provenance?: Record<string, unknown> | null;
  },
): Promise<GatewayWorkflowRunProducedFileResource> {
  const response = await fetch(
    `${context.gatewayApiUrl.replace(/\/$/, "")}/api/v1/workflow-runs/${encodeURIComponent(context.workflowRunId)}/produced-files`,
    {
      method: "POST",
      headers: {
        "x-bpane-automation-access-token": context.authToken,
        "x-bpane-workflow-workspace-id": request.workspaceId,
        "x-bpane-file-name": request.fileName,
        ...(request.mediaType ? { "Content-Type": request.mediaType } : {}),
        ...(request.provenance
          ? {
              "x-bpane-file-provenance": JSON.stringify(request.provenance),
            }
          : {}),
      },
      body: Buffer.from(request.bytes),
    },
  );
  if (!response.ok) {
    let message = `${response.status} ${response.statusText}`.trim();
    try {
      const payload = (await response.json()) as { error?: string };
      if (payload?.error) {
        message = payload.error;
      }
    } catch {
      // Ignore malformed error bodies.
    }
    throw new Error(`failed to upload workflow produced file: ${message}`);
  }
  return (await response.json()) as GatewayWorkflowRunProducedFileResource;
}

function ensureJsonSerializable(value: unknown): unknown {
  if (typeof value === "undefined") {
    return null;
  }
  return JSON.parse(JSON.stringify(value));
}

main().catch((error) => {
  const message = error instanceof Error ? error.stack ?? error.message : String(error);
  console.error(message);
  process.exitCode = 1;
});
