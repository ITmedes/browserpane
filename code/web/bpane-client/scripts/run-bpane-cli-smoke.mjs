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
  let projectId = '';
  let contextId = '';
  let clonedContextId = '';
  let importedContextId = '';
  let servicePrincipalId = '';
  let identityMappingId = '';
  let servicePrincipalMappingId = '';
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

    const identity = runBpaneCli(['identity', 'me'], cliEnv);
    if (!identity.subject || !identity.issuer || !['user', 'service_principal', 'legacy_dev_token'].includes(identity.principal_type)) {
      throw new Error(`CLI identity me returned unexpected data: ${JSON.stringify(identity)}`);
    }

    const servicePrincipal = ensureBridgeServicePrincipal(cliEnv, bridge, runLabel);
    servicePrincipalId = servicePrincipal.id;
    if (!servicePrincipalId || servicePrincipal.client_id !== bridge.clientId || servicePrincipal.state !== 'active') {
      throw new Error(`CLI service-principal ensure returned unexpected data: ${JSON.stringify(servicePrincipal)}`);
    }

    const listedServicePrincipals = runBpaneCli(['service-principal', 'list'], cliEnv);
    if (
      !Array.isArray(listedServicePrincipals.service_principals)
      || !listedServicePrincipals.service_principals.some((item) => item.id === servicePrincipalId)
    ) {
      throw new Error(`CLI service-principal list did not include ${servicePrincipalId}.`);
    }

    const fetchedServicePrincipal = runBpaneCli(['service-principal', 'get', servicePrincipalId], cliEnv);
    if (fetchedServicePrincipal.id !== servicePrincipalId || fetchedServicePrincipal.labels?.run_id !== runLabel) {
      throw new Error(`CLI service-principal get returned unexpected data: ${JSON.stringify(fetchedServicePrincipal)}`);
    }

    const disabledServicePrincipal = runBpaneCli(['service-principal', 'disable', servicePrincipalId], cliEnv);
    if (disabledServicePrincipal.state !== 'disabled') {
      throw new Error(`CLI service-principal disable did not persist disabled state: ${JSON.stringify(disabledServicePrincipal)}`);
    }

    const enabledServicePrincipal = runBpaneCli([
      'service-principal',
      'update',
      servicePrincipalId,
      '--state',
      'active',
    ], cliEnv);
    if (enabledServicePrincipal.state !== 'active') {
      throw new Error(`CLI service-principal update did not re-enable the principal: ${JSON.stringify(enabledServicePrincipal)}`);
    }

    const project = runBpaneCli([
      'project',
      'create',
      `support-tenant-${runLabel}`,
      '--description',
      'Operator CLI smoke project',
      '--label',
      'suite=bpane-cli-smoke',
      '--label',
      `run_id=${runLabel}`,
      '--max-active-sessions',
      '3',
      '--max-active-workflow-runs',
      '4',
      '--max-retained-storage-bytes',
      '1073741824',
    ], cliEnv);
    projectId = project.id;
    if (!projectId || project.quotas?.max_active_sessions !== 3 || project.state !== 'active') {
      throw new Error(`CLI project create returned an invalid project: ${JSON.stringify(project)}`);
    }

    const listedProjects = runBpaneCli(['project', 'list'], cliEnv);
    if (!Array.isArray(listedProjects.projects) || !listedProjects.projects.some((item) => item.id === projectId)) {
      throw new Error(`CLI project list did not include ${projectId}.`);
    }

    const fetchedProject = runBpaneCli(['project', 'get', projectId], cliEnv);
    if (fetchedProject.id !== projectId || fetchedProject.labels?.run_id !== runLabel) {
      throw new Error(`CLI project get returned unexpected project data: ${JSON.stringify(fetchedProject)}`);
    }

    const projectUsage = runBpaneCli(['project', 'usage', projectId], cliEnv);
    if (projectUsage.project_id !== projectId || projectUsage.active_sessions !== 0) {
      throw new Error(`CLI project usage returned unexpected data: ${JSON.stringify(projectUsage)}`);
    }

    const identityMapping = runBpaneCli([
      'identity-mapping',
      'create',
      `operator-user-${runLabel}`,
      '--description',
      'Operator CLI smoke identity mapping',
      '--kind',
      'user',
      '--issuer',
      identity.issuer,
      '--external-id',
      identity.subject,
      '--project-id',
      projectId,
      '--label',
      'suite=bpane-cli-smoke',
      '--label',
      `run_id=${runLabel}`,
      '--scope',
      'session:create',
    ], cliEnv);
    identityMappingId = identityMapping.id;
    if (
      !identityMappingId
      || identityMapping.kind !== 'user'
      || identityMapping.external_id !== identity.subject
      || identityMapping.project_id !== projectId
      || identityMapping.state !== 'active'
    ) {
      throw new Error(`CLI identity-mapping create returned unexpected data: ${JSON.stringify(identityMapping)}`);
    }

    const listedIdentityMappings = runBpaneCli(['identity-mapping', 'list'], cliEnv);
    if (
      !Array.isArray(listedIdentityMappings.identity_mappings)
      || !listedIdentityMappings.identity_mappings.some((item) => item.id === identityMappingId)
    ) {
      throw new Error(`CLI identity-mapping list did not include ${identityMappingId}.`);
    }

    const fetchedIdentityMapping = runBpaneCli(['identity-mapping', 'get', identityMappingId], cliEnv);
    if (fetchedIdentityMapping.id !== identityMappingId || fetchedIdentityMapping.labels?.run_id !== runLabel) {
      throw new Error(`CLI identity-mapping get returned unexpected data: ${JSON.stringify(fetchedIdentityMapping)}`);
    }

    const disabledIdentityMapping = runBpaneCli(['identity-mapping', 'disable', identityMappingId], cliEnv);
    if (disabledIdentityMapping.state !== 'disabled') {
      throw new Error(`CLI identity-mapping disable did not persist disabled state: ${JSON.stringify(disabledIdentityMapping)}`);
    }

    const enabledIdentityMapping = runBpaneCli([
      'identity-mapping',
      'update',
      identityMappingId,
      '--state',
      'active',
    ], cliEnv);
    if (enabledIdentityMapping.state !== 'active') {
      throw new Error(`CLI identity-mapping update did not re-enable the mapping: ${JSON.stringify(enabledIdentityMapping)}`);
    }

    const servicePrincipalMapping = runBpaneCli([
      'identity-mapping',
      'create',
      `mcp-bridge-${runLabel}`,
      '--description',
      'Operator CLI smoke service-principal project mapping',
      '--kind',
      'service_principal',
      '--issuer',
      bridge.issuer,
      '--external-id',
      bridge.clientId,
      '--service-principal-id',
      servicePrincipalId,
      '--project-id',
      projectId,
      '--label',
      'suite=bpane-cli-smoke',
      '--label',
      `run_id=${runLabel}`,
      '--scope',
      'session:delegate',
    ], cliEnv);
    servicePrincipalMappingId = servicePrincipalMapping.id;
    if (
      !servicePrincipalMappingId
      || servicePrincipalMapping.kind !== 'service_principal'
      || servicePrincipalMapping.service_principal_id !== servicePrincipalId
      || servicePrincipalMapping.external_id !== bridge.clientId
    ) {
      throw new Error(`CLI service-principal identity mapping returned unexpected data: ${JSON.stringify(servicePrincipalMapping)}`);
    }

    const egressProfile = runBpaneCli([
      'egress-profile',
      'create',
      `eu-support-egress-${runLabel}`,
      '--description',
      'Operator CLI smoke egress profile',
      '--label',
      'suite=bpane-cli-smoke',
      '--label',
      `run_id=${runLabel}`,
      '--proxy-url',
      'https://proxy.example:8443',
      '--bypass-rule',
      'localhost',
      '--bypass-rule',
      '*.internal.example',
      '--custom-ca-ref',
      'vault://pki/browserpane/eu-support',
      '--custom-ca-name',
      'EU support CA',
    ], cliEnv);
    const egressProfileId = egressProfile.id;
    if (!egressProfileId || egressProfile.effective?.proxy_configured !== true) {
      throw new Error(`CLI egress-profile create returned an invalid profile: ${JSON.stringify(egressProfile)}`);
    }

    const listedEgressProfiles = runBpaneCli(['egress-profile', 'list'], cliEnv);
    if (!Array.isArray(listedEgressProfiles.profiles) || !listedEgressProfiles.profiles.some((item) => item.id === egressProfileId)) {
      throw new Error(`CLI egress-profile list did not include ${egressProfileId}.`);
    }

    const fetchedEgressProfile = runBpaneCli(['egress-profile', 'get', egressProfileId], cliEnv);
    if (fetchedEgressProfile.id !== egressProfileId || fetchedEgressProfile.bypass_rules?.length !== 2) {
      throw new Error(`CLI egress-profile get returned unexpected profile data: ${JSON.stringify(fetchedEgressProfile)}`);
    }
    const egressProfileDiagnostics = runBpaneCli(['egress-profile', 'diagnostics', egressProfileId], cliEnv);
    if (egressProfileDiagnostics.profile_id !== egressProfileId || egressProfileDiagnostics.health !== 'ready') {
      throw new Error(`CLI egress-profile diagnostics returned unexpected data: ${JSON.stringify(egressProfileDiagnostics)}`);
    }
    const egressProfileProbe = runBpaneCli([
      'egress-profile',
      'diagnostics',
      'probe',
      egressProfileId,
      '--probe-timeout-ms',
      '1000',
    ], cliEnv);
    if (
      egressProfileProbe.profile_id !== egressProfileId
      || typeof egressProfileProbe.proof?.profile_reachability_collected !== 'boolean'
      || (
        !egressProfileProbe.proof.profile_reachability_healthy
        && !egressProfileProbe.proof.profile_reachability_failure
      )
    ) {
      throw new Error(`CLI egress-profile diagnostics probe returned unexpected data: ${JSON.stringify(egressProfileProbe)}`);
    }

    const template = runBpaneCli([
      'session-template',
      'create',
      `customer-debug-${runLabel}`,
      '--description',
      'Operator CLI smoke template',
      '--project-id',
      projectId,
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
      '--locale',
      'de-DE',
      '--language',
      'de-DE',
      '--language',
      'en-US',
      '--timezone',
      'Europe/Berlin',
      '--egress-profile-id',
      egressProfileId,
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
      '--max-profile-storage-bytes',
      '67108864',
    ], cliEnv);
    contextId = browserContext.id;
    if (
      !contextId
      || browserContext.persistence_mode !== 'reusable'
      || browserContext.retention_sec !== 604800
      || browserContext.max_profile_storage_bytes !== 67108864
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

    const exportPath = path.join(configDir, 'browser-context-export.zip');
    const exportedContext = runBpaneCli([
      'browser-context',
      'export',
      contextId,
      '--output',
      exportPath,
    ], cliEnv);
    const exportBytes = await fs.readFile(exportPath);
    if (
      exportedContext.context_id !== contextId
      || exportedContext.byte_count < 64
      || exportBytes[0] !== 0x50
      || exportBytes[1] !== 0x4b
    ) {
      throw new Error(`CLI browser-context export did not write a zip archive: ${JSON.stringify(exportedContext)}`);
    }

    const importedContext = runBpaneCli([
      'browser-context',
      'import',
      '--input',
      exportPath,
      '--name',
      `support-profile-import-${runLabel}`,
      '--label',
      'suite=bpane-cli-smoke',
      '--label',
      'imported=true',
    ], cliEnv);
    importedContextId = importedContext.id;
    if (
      !importedContextId
      || importedContextId === contextId
      || importedContext.name !== `support-profile-import-${runLabel}`
      || importedContext.labels?.imported !== 'true'
      || importedContext.persistence_mode !== 'reusable'
    ) {
      throw new Error(`CLI browser-context import returned an invalid context: ${JSON.stringify(importedContext)}`);
    }

    const clonedContext = runBpaneCli([
      'browser-context',
      'clone',
      contextId,
      `support-profile-copy-${runLabel}`,
      '--description',
      'Operator CLI smoke context clone',
      '--label',
      'suite=bpane-cli-smoke',
      '--label',
      'copy=sandbox',
    ], cliEnv);
    clonedContextId = clonedContext.id;
    if (
      !clonedContextId
      || clonedContextId === contextId
      || clonedContext.name !== `support-profile-copy-${runLabel}`
      || clonedContext.labels?.copy !== 'sandbox'
      || clonedContext.persistence_mode !== 'reusable'
    ) {
      throw new Error(`CLI browser-context clone returned an invalid context: ${JSON.stringify(clonedContext)}`);
    }

    const created = runBpaneCli([
      'session',
      'create',
      '--template-id',
      templateId,
      '--project-id',
      projectId,
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
      || created.project_id !== projectId
      || created.project?.id !== projectId
      || created.admission?.state !== 'allowed'
      || created.browser_context?.mode !== 'reusable'
      || created.browser_context?.context_id !== contextId
      || created.network_identity?.locale !== 'de-DE'
      || created.network_identity?.timezone !== 'Europe/Berlin'
      || created.network_identity?.egress_profile_id !== egressProfileId
      || created.effective_egress?.profile_id !== egressProfileId
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

    const sessionEgressDiagnostics = runBpaneCli(['session', 'egress-diagnostics', sessionId], cliEnv);
    if (
      sessionEgressDiagnostics.profile_id !== egressProfileId
      || sessionEgressDiagnostics.proof_level !== 'runtime_launch_metadata'
    ) {
      throw new Error(`CLI session egress-diagnostics returned unexpected data: ${JSON.stringify(sessionEgressDiagnostics)}`);
    }
    const sessionEgressProbe = runBpaneCli([
      'session',
      'egress-diagnostics',
      'probe',
      sessionId,
      '--probe-public-ip-url',
      'https://example.com/',
      '--probe-tls-url',
      'https://example.com/',
      '--probe-timeout-ms',
      '1000',
    ], cliEnv);
    if (
      sessionEgressProbe.profile_id !== egressProfileId
      || typeof sessionEgressProbe.proof?.active_probe_collected !== 'boolean'
      || (!sessionEgressProbe.proof.active_probe_collected && !sessionEgressProbe.proof.last_failure_reason)
    ) {
      throw new Error(`CLI session egress-diagnostics probe returned unexpected data: ${JSON.stringify(sessionEgressProbe)}`);
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

    const accessReview = runBpaneCli(['identity', 'access-review'], cliEnv);
    if (
      accessReview.principal?.subject !== identity.subject
      || accessReview.resource_counts?.sessions < 1
      || accessReview.resource_counts?.projects < 1
      || accessReview.resource_counts?.service_principals < 1
      || accessReview.resource_counts?.identity_mappings < 2
      || !Array.isArray(accessReview.projects)
      || !accessReview.projects.some((item) => item.id === projectId)
      || !Array.isArray(accessReview.service_principals)
      || !accessReview.service_principals.some((item) => item.id === servicePrincipalId && item.delegated_session_count >= 1)
      || !Array.isArray(accessReview.identity_mappings)
      || !accessReview.identity_mappings.some((item) => item.id === identityMappingId && item.effective_for_principal === true)
      || !accessReview.identity_mappings.some((item) => item.id === servicePrincipalMappingId)
      || !Array.isArray(accessReview.unmapped_principal_signals)
      || !Array.isArray(accessReview.delegated_principals)
      || !accessReview.delegated_principals.some((item) => item.client_id === bridge.clientId && item.registered === true)
    ) {
      throw new Error(`CLI identity access-review returned unexpected data: ${JSON.stringify(accessReview)}`);
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
    const deletedCloneContext = runBpaneCli(['browser-context', 'delete', clonedContextId], cliEnv);
    if (deletedCloneContext.id !== clonedContextId || deletedCloneContext.state !== 'deleted') {
      throw new Error(`CLI browser-context delete did not soft-delete the cloned context: ${JSON.stringify(deletedCloneContext)}`);
    }
    clonedContextId = '';
    const deletedImportedContext = runBpaneCli(['browser-context', 'delete', importedContextId], cliEnv);
    if (deletedImportedContext.id !== importedContextId || deletedImportedContext.state !== 'deleted') {
      throw new Error(`CLI browser-context delete did not soft-delete the imported context: ${JSON.stringify(deletedImportedContext)}`);
    }
    importedContextId = '';
    const deletedContext = runBpaneCli(['browser-context', 'delete', contextId], cliEnv);
    if (deletedContext.id !== contextId || deletedContext.state !== 'deleted') {
      throw new Error(`CLI browser-context delete did not soft-delete the context: ${JSON.stringify(deletedContext)}`);
    }
    contextId = '';

    const archivedProject = runBpaneCli(['project', 'archive', projectId], cliEnv);
    if (archivedProject.id !== projectId || archivedProject.state !== 'archived') {
      throw new Error(`CLI project archive did not archive the project: ${JSON.stringify(archivedProject)}`);
    }
    projectId = '';

    log('Operator CLI smoke passed.');
  } finally {
    if (clonedContextId && accessToken) {
      await fetch(`${apiOrigin(options)}/api/v1/browser-contexts/${clonedContextId}`, {
        method: 'DELETE',
        headers: { authorization: `Bearer ${accessToken}` },
      }).catch(() => undefined);
    }
    if (importedContextId && accessToken) {
      await fetch(`${apiOrigin(options)}/api/v1/browser-contexts/${importedContextId}`, {
        method: 'DELETE',
        headers: { Authorization: `Bearer ${accessToken}` },
      }).catch(() => {});
    }
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
    if (projectId && accessToken) {
      await archiveProject(options, accessToken, projectId).catch(() => {});
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

function ensureBridgeServicePrincipal(cliEnv, bridge, runLabel) {
  const listed = runBpaneCli(['service-principal', 'list'], cliEnv);
  const existing = listed.service_principals?.find((item) => (
    item.client_id === bridge.clientId && item.issuer === bridge.issuer
  ));
  const args = existing
    ? [
        'service-principal',
        'update',
        existing.id,
        '--name',
        `mcp-bridge-${runLabel}`,
        '--description',
        'Operator CLI smoke service principal',
        '--state',
        'active',
      ]
    : [
        'service-principal',
        'create',
        `mcp-bridge-${runLabel}`,
        '--description',
        'Operator CLI smoke service principal',
        '--client-id',
        bridge.clientId,
        '--issuer',
        bridge.issuer,
      ];
  return runBpaneCli([
    ...args,
    '--label',
    'suite=bpane-cli-smoke',
    '--label',
    `run_id=${runLabel}`,
    '--scope',
    'session:delegate',
  ], cliEnv);
}

async function clearMcpBridge(options) {
  const bridge = await loadMcpBridgeConfig(options);
  const response = await fetch(bridge.controlUrl, { method: 'DELETE' });
  if (!response.ok && response.status !== 404) {
    const detail = await response.text().catch(() => '');
    throw new Error(`Could not clear MCP bridge control session: HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
}

async function archiveProject(options, accessToken, projectId) {
  const projectResponse = await fetch(`${apiOrigin(options)}/api/v1/projects/${projectId}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (!projectResponse.ok) {
    return;
  }
  const project = await projectResponse.json();
  await fetch(`${apiOrigin(options)}/api/v1/projects/${projectId}`, {
    method: 'PUT',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      name: project.name,
      description: project.description,
      labels: project.labels ?? {},
      quotas: project.quotas ?? {},
      state: 'archived',
    }),
  });
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
