import { lookup } from "node:dns/promises";
import { promises as fs } from "node:fs";
import net from "node:net";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { chromium, type Browser, type BrowserContext, type Page } from "playwright-core";
import type {
  WorkflowResolvedCredentialBinding,
  WorkflowRunnerContext,
  WorkflowRunnerCredentialBinding,
  WorkflowRunnerWorkspaceInput,
} from "./types.js";

type WorkflowExecutionContext = {
  browser: Browser;
  context: BrowserContext;
  page: Page;
  credentialBindings: WorkflowRunnerCredentialBinding[];
  credentials: {
    bindings: WorkflowRunnerCredentialBinding[];
    load: (bindingId: string, targetOrigin: string) => Promise<WorkflowResolvedCredentialBinding>;
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
    const page = await resolveExecutionPage(browser);
    const module = (await import(pathToFileURL(context.entrypointPath).href)) as WorkflowEntrypointModule;
    const execute = resolveEntrypointFunction(module, context.entrypointPath);
    const output = await execute({
      browser,
      context: page.context(),
      page,
      credentialBindings: context.credentialBindings,
      credentials: {
        bindings: context.credentialBindings,
        load: async (bindingId: string, targetOrigin: string) =>
          loadResolvedCredentialBinding(context, bindingId, targetOrigin),
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

async function resolveExecutionPage(browser: Browser): Promise<Page> {
  const context = browser.contexts()[0];
  if (!context) {
    throw new Error("workflow session did not expose a browser context over CDP");
  }
  const existingPage = context.pages()[0];
  if (existingPage) {
    return existingPage;
  }
  return context.newPage();
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

function normalizeOrigin(value: string): string {
  return new URL(value).origin;
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
