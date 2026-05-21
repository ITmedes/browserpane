import process from 'node:process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

import { chromium } from 'playwright-core';

import {
  cleanupAdminBeforeRun,
  cleanupAdminSmoke,
  ensureAdminLoggedIn,
  getAdminAccessToken,
} from './admin-smoke-lib.mjs';
import {
  DEFAULTS,
  apiOrigin,
  createLogger,
  fetchAuthConfig,
  launchChrome,
  parseSmokeArgs,
} from './workflow-smoke-lib.mjs';

const log = createLogger('bpane-cli-smoke');

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-bpane-cli-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin/`;
  }

  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1360, height: 920 } });
  const page = await context.newPage();
  let accessToken = '';
  let sessionId = '';
  let contextId = '';
  let configDir = '';

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    await cleanupAdminBeforeRun(page, options, log);
    accessToken = await getAdminAccessToken(page);
    if (!accessToken) {
      throw new Error('Failed to acquire an admin access token.');
    }

    const bridge = await loadMcpBridgeConfig(options);
    configDir = await fs.mkdtemp(path.join(os.tmpdir(), 'bpane-cli-smoke-'));
    const configPath = path.join(configDir, 'config.json');

    const cliEnv = {
      ...process.env,
      BPANE_CONFIG: configPath,
      BPANE_PROFILE: 'smoke',
    };
    const runLabel = `bpane-cli-smoke-${Date.now()}`;

    const initialized = runBpaneCli([
      'profile',
      'init',
      'smoke',
      '--base-url',
      apiOrigin(options),
      '--access-token',
      accessToken,
      '--mcp-control-url',
      bridge.controlUrl,
      '--mcp-client-id',
      bridge.clientId,
      '--mcp-issuer',
      bridge.issuer ?? '',
      '--mcp-display-name',
      bridge.displayName ?? '',
      '--save-token',
      '--set-default',
    ], cliEnv);
    if (initialized.profile !== 'smoke' || initialized.token_saved !== true) {
      throw new Error('CLI profile init did not create the smoke profile.');
    }

    const profiles = runBpaneCli(['profile', 'list'], cliEnv);
    if (!profiles.profiles?.includes('smoke')) {
      throw new Error('CLI profile list did not include the smoke profile.');
    }
    const profile = runBpaneCli(['profile', 'show'], cliEnv);
    if (profile.profile !== 'smoke' || profile.values?.base_url !== apiOrigin(options)) {
      throw new Error('CLI profile show did not return the smoke profile.');
    }

    const template = runBpaneCli([
      'session-template',
      'create',
      `customer-debug-${runLabel}`,
      '--description',
      'Operator CLI smoke template',
      '--label',
      'suite=bpane-cli-smoke',
      '--default-label',
      'team=support',
      '--default-label',
      'purpose=debug',
      '--owner-mode',
      'collaborative',
      '--idle-timeout-sec',
      '1800',
      '--integration-json',
      JSON.stringify({ source: 'bpane-cli-smoke-template' }),
      '--recording-mode',
      'manual',
      '--recording-retention-sec',
      '86400',
    ], cliEnv);
    const templateId = template.id;
    if (!templateId || template.version !== 1) {
      throw new Error(`CLI session-template create returned an invalid template: ${JSON.stringify(template)}`);
    }

    const listedTemplates = runBpaneCli(['session-template', 'list'], cliEnv);
    if (!Array.isArray(listedTemplates.templates) || !listedTemplates.templates.some((item) => item.id === templateId)) {
      throw new Error(`CLI session-template list did not include ${templateId}.`);
    }

    const fetchedTemplate = runBpaneCli(['session-template', 'get', templateId], cliEnv);
    if (fetchedTemplate.id !== templateId || fetchedTemplate.defaults?.labels?.team !== 'support') {
      throw new Error(`CLI session-template get returned unexpected template data: ${JSON.stringify(fetchedTemplate)}`);
    }

    const updatedTemplate = runBpaneCli([
      'session-template',
      'update',
      templateId,
      '--name',
      `customer-debug-${runLabel}`,
      '--description',
      'Operator CLI smoke template updated',
      '--default-label',
      'team=support',
      '--default-label',
      'purpose=debug',
      '--default-label',
      'tier=gold',
      '--idle-timeout-sec',
      '1800',
      '--recording-mode',
      'manual',
      '--recording-retention-sec',
      '86400',
    ], cliEnv);
    if (updatedTemplate.id !== templateId || updatedTemplate.version !== 2 || updatedTemplate.defaults?.labels?.tier !== 'gold') {
      throw new Error(`CLI session-template update did not increment the template version: ${JSON.stringify(updatedTemplate)}`);
    }

    const browserContext = runBpaneCli([
      'browser-context',
      'create',
      `support-profile-${runLabel}`,
      '--description',
      'Operator CLI smoke context',
      '--label',
      'suite=bpane-cli-smoke',
      '--retention-sec',
      '604800',
    ], cliEnv);
    contextId = browserContext.id;
    if (
      !contextId
      || browserContext.persistence_mode !== 'reusable'
      || browserContext.retention_sec !== 604800
      || !browserContext.retention_expires_at
      || browserContext.state !== 'ready'
    ) {
      throw new Error(`CLI browser-context create returned an invalid context: ${JSON.stringify(browserContext)}`);
    }

    const listedContexts = runBpaneCli(['browser-context', 'list'], cliEnv);
    if (!Array.isArray(listedContexts.contexts) || !listedContexts.contexts.some((item) => item.id === contextId)) {
      throw new Error(`CLI browser-context list did not include ${contextId}.`);
    }

    const fetchedContext = runBpaneCli(['browser-context', 'get', contextId], cliEnv);
    if (fetchedContext.id !== contextId || fetchedContext.labels?.suite !== 'bpane-cli-smoke') {
      throw new Error(`CLI browser-context get returned unexpected context data: ${JSON.stringify(fetchedContext)}`);
    }

    const created = runBpaneCli([
      'session',
      'create',
      '--template-id',
      templateId,
      '--browser-context-id',
      contextId,
      '--label',
      'suite=bpane-cli-smoke',
      '--label',
      `run_id=${runLabel}`,
      '--integration-json',
      JSON.stringify({ ticket: runLabel }),
    ], cliEnv);
    sessionId = created.id;
    if (!sessionId) {
      throw new Error('CLI session create did not return an id.');
    }
    if (
      created.template_id !== templateId
      || created.browser_context?.mode !== 'reusable'
      || created.browser_context?.context_id !== contextId
      || created.labels?.team !== 'support'
      || created.labels?.tier !== 'gold'
      || created.labels?.run_id !== runLabel
      || created.integration_context?.ticket !== runLabel
      || created.recording?.mode !== 'manual'
    ) {
      throw new Error(`CLI session create did not apply template defaults: ${JSON.stringify(created)}`);
    }

    const listed = runBpaneCli(['session', 'list'], cliEnv);
    if (!Array.isArray(listed.sessions) || !listed.sessions.some((session) => session.id === sessionId)) {
      throw new Error(`CLI session list did not include ${sessionId}.`);
    }

    const filteredList = runBpaneCli([
      'session',
      'list',
      '--template-id',
      templateId,
      '--label',
      `run_id=${runLabel}`,
      '--integration',
      `ticket=${runLabel}`,
      '--runtime-state',
      'not_started',
      '--limit',
      '1',
    ], cliEnv);
    if (!Array.isArray(filteredList.sessions) || filteredList.sessions.length !== 1 || filteredList.sessions[0]?.id !== sessionId) {
      throw new Error('CLI filtered session list did not return the smoke session.');
    }

    const fetched = runBpaneCli(['session', 'get', sessionId], cliEnv);
    if (fetched.id !== sessionId) {
      throw new Error(`CLI session get returned ${fetched.id ?? 'no id'} instead of ${sessionId}.`);
    }

    const status = runBpaneCli(['session', 'status', sessionId], cliEnv);
    if (!status.connection_counts) {
      throw new Error('CLI session status did not return connection_counts.');
    }

    const accessTicket = runBpaneCli(['session', 'access-token', sessionId], cliEnv);
    if (accessTicket.token_type !== 'session_connect_ticket' || !accessTicket.token) {
      throw new Error('CLI session access-token did not mint a connect ticket.');
    }

    const automationAccess = runBpaneCli(['session', 'automation-access', sessionId], cliEnv);
    if (automationAccess.token_type !== 'session_automation_access_token' || !automationAccess.automation?.endpoint_url) {
      throw new Error('CLI session automation-access did not mint automation access.');
    }

    const disconnected = runBpaneCli(['session', 'disconnect-all', sessionId], cliEnv);
    if (!disconnected.connection_counts) {
      throw new Error('CLI session disconnect-all did not return session status.');
    }

    const health = runBpaneCli(['mcp', 'health'], cliEnv);
    if (health.status !== 'ok') {
      throw new Error(`CLI MCP health returned ${health.status ?? 'no status'}.`);
    }

    const authorized = runBpaneCli(['mcp', 'authorize', sessionId], cliEnv);
    if (authorized.id !== sessionId) {
      throw new Error('CLI MCP authorize did not return the session.');
    }

    const defaulted = runBpaneCli(['mcp', 'set-default', sessionId], cliEnv);
    if (defaulted.session?.id !== sessionId) {
      throw new Error('CLI MCP set-default did not select the smoke session.');
    }

    const doctor = runBpaneCli(['mcp', 'doctor', sessionId], cliEnv);
    if (doctor.ok !== true) {
      throw new Error(`CLI MCP doctor reported issues: ${JSON.stringify(doctor.issues)}`);
    }

    const preflight = runBpaneCli(['mcp', 'preflight', sessionId], cliEnv);
    if (preflight.ok !== true) {
      throw new Error(`CLI MCP preflight reported issues: ${JSON.stringify(preflight.issues)}`);
    }

    const cleared = runBpaneCli(['mcp', 'clear-default'], cliEnv);
    if (cleared.ok !== true) {
      throw new Error('CLI MCP clear-default did not return ok=true.');
    }

    const revoked = runBpaneCli(['mcp', 'revoke', sessionId], cliEnv);
    if (revoked.id !== sessionId) {
      throw new Error('CLI MCP revoke did not return the session.');
    }

    const repaired = runBpaneCli(['mcp', 'repair', sessionId], cliEnv);
    if (repaired.ok !== true || repaired.actions?.some((action) => action.ok !== true)) {
      throw new Error(`CLI MCP repair did not align delegation: ${JSON.stringify(repaired)}`);
    }

    const repairedDoctor = runBpaneCli(['mcp', 'doctor', sessionId], cliEnv);
    if (repairedDoctor.ok !== true) {
      throw new Error(`CLI MCP doctor reported issues after repair: ${JSON.stringify(repairedDoctor.issues)}`);
    }

    const repairedPreflight = runBpaneCli(['mcp', 'preflight', sessionId], cliEnv);
    if (repairedPreflight.ok !== true) {
      throw new Error(`CLI MCP preflight reported issues after repair: ${JSON.stringify(repairedPreflight.issues)}`);
    }

    const clearedAfterRepair = runBpaneCli(['mcp', 'clear-default'], cliEnv);
    if (clearedAfterRepair.ok !== true) {
      throw new Error('CLI MCP clear-default after repair did not return ok=true.');
    }

    const stopped = runBpaneCli(['session', 'stop', sessionId], cliEnv);
    if (stopped.id !== sessionId || stopped.state !== 'stopped') {
      throw new Error('CLI session stop did not stop the session.');
    }

    const killed = runBpaneCli(['session', 'kill', sessionId], cliEnv);
    if (killed.id !== sessionId || killed.state !== 'stopped') {
      throw new Error('CLI session kill did not stop the session.');
    }

    const cleanupDryRun = runBpaneCli(['session', 'cleanup', '--label', `run_id=${runLabel}`], cliEnv);
    if (
      cleanupDryRun.dry_run !== true
      || cleanupDryRun.candidate_count < 1
      || !cleanupDryRun.planned_actions?.includes('revoke-automation-owner')
      || !cleanupDryRun.planned_actions?.includes('disconnect-all')
      || !cleanupDryRun.planned_actions?.includes('kill')
    ) {
      throw new Error('CLI session cleanup dry-run did not plan the default stopped-session cleanup actions.');
    }

    const cleanupConfirmed = runBpaneCli(['session', 'cleanup', '--label', `run_id=${runLabel}`, '--confirm'], cliEnv);
    if (cleanupConfirmed.dry_run !== false || cleanupConfirmed.result_count < 1 || cleanupConfirmed.failure_count !== 0) {
      throw new Error(`CLI session cleanup confirm did not execute cleanup operations: ${JSON.stringify(cleanupConfirmed)}`);
    }
    sessionId = '';
    const deletedContext = runBpaneCli(['browser-context', 'delete', contextId], cliEnv);
    if (deletedContext.id !== contextId || deletedContext.state !== 'deleted') {
      throw new Error(`CLI browser-context delete did not soft-delete the context: ${JSON.stringify(deletedContext)}`);
    }
    contextId = '';

    log('Operator CLI smoke passed.');
  } finally {
    if (contextId && accessToken) {
      await fetch(`${apiOrigin(options)}/api/v1/browser-contexts/${contextId}`, {
        method: 'DELETE',
        headers: { Authorization: `Bearer ${accessToken}` },
      }).catch(() => {});
    }
    if (sessionId && accessToken) {
      await clearMcpBridge(options).catch(() => {});
      await fetch(`${apiOrigin(options)}/api/v1/sessions/${sessionId}/automation-owner`, {
        method: 'DELETE',
        headers: { Authorization: `Bearer ${accessToken}` },
      }).catch(() => {});
      await fetch(`${apiOrigin(options)}/api/v1/sessions/${sessionId}/kill`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${accessToken}` },
      }).catch(() => {});
    }
    await cleanupAdminSmoke(page, options, log);
    if (configDir) {
      await fs.rm(configDir, { recursive: true, force: true }).catch(() => {});
    }
    await context.close();
    await browser.close();
  }
}

async function loadMcpBridgeConfig(options) {
  const config = await fetchAuthConfig({ ...options, pageUrl: apiOrigin(options) });
  const bridge = config?.mcpBridge;
  if (!bridge?.controlUrl || !bridge.clientId) {
    throw new Error('Operator CLI smoke requires auth-config mcpBridge metadata.');
  }
  return bridge;
}

async function clearMcpBridge(options) {
  const bridge = await loadMcpBridgeConfig(options);
  const response = await fetch(bridge.controlUrl, { method: 'DELETE' });
  if (!response.ok && response.status !== 404) {
    const detail = await response.text().catch(() => '');
    throw new Error(`Could not clear MCP bridge control session: HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
}

function runBpaneCli(args, env) {
  const cliPath = path.join(process.cwd(), 'scripts', 'bpane-cli.mjs');
  const result = spawnSync(process.execPath, [cliPath, ...args], {
    cwd: process.cwd(),
    env,
    encoding: 'utf8',
    maxBuffer: 10 * 1024 * 1024,
  });
  if (result.status !== 0) {
    const detail = result.stderr.trim() || result.stdout.trim() || result.error?.message || 'unknown error';
    throw new Error(`bpane CLI failed for ${args.join(' ')} with code ${result.status ?? 'unknown'}: ${detail}`);
  }
  const stdout = result.stdout.trim();
  if (!stdout) {
    return null;
  }
  try {
    return JSON.parse(stdout);
  } catch (error) {
    throw new Error(
      `bpane CLI returned invalid JSON for ${args.join(' ')}: ${
        error instanceof Error ? error.message : String(error)
      }; stdout length=${stdout.length}; tail=${stdout.slice(-240)}`,
    );
  }
}

run().catch((error) => {
  console.error(error instanceof Error ? error.stack ?? error.message : String(error));
  process.exit(1);
});
