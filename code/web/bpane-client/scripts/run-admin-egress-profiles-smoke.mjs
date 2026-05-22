import process from 'node:process';
import { spawnSync } from 'node:child_process';
import { chromium } from 'playwright-core';
import {
  ensureAdminLoggedIn,
  getAdminAccessToken,
  openAdminTab,
} from './admin-smoke-lib.mjs';
import {
  DEFAULTS,
  apiOrigin,
  createLogger,
  deleteSession,
  fetchJson,
  launchChrome,
  parseSmokeArgs,
  poll,
} from './workflow-smoke-lib.mjs';

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-egress-profiles-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin/`;
  }
  const log = createLogger('admin-egress-profiles-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();
  const runLabel = Date.now();
  const observerSince = new Date(Date.now() - 1000).toISOString();
  let sessionIds = [];

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    const accessToken = await getAdminAccessToken(page);
    if (!accessToken) {
      throw new Error('No admin access token available after login.');
    }

    await page.goto(options.pageUrl, { waitUntil: 'domcontentloaded' });
    await openAdminTab(page, 'egress');
    await page.getByTestId('egress-profile-catalog').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });

    const proxyProfile = await createProfileThroughUi(page, options, {
      name: `smoke-proxy-${runLabel}`,
      description: 'Admin smoke metadata-only proxy profile',
      labels: 'suite=admin-egress\nmode=proxy',
      proxyUrl: 'http://bpane-egress-observer:3128',
      bypassRules: 'localhost\n*.local',
      mode: 'metadata_only',
    });
    const tlsProfile = await createProfileThroughUi(page, options, {
      name: `smoke-tls-${runLabel}`,
      description: 'Admin smoke TLS-intercept profile',
      labels: 'suite=admin-egress\nmode=tls',
      proxyUrl: 'http://bpane-egress-tls-observer:3129',
      bypassRules: 'localhost\n*.local',
      customCaRef: 'file:///workspace/dev/egress-ca.pem',
      customCaName: 'BrowserPane Local Egress Test CA',
      logSinkRef: 'siem://browserpane/local-egress',
      logSinkName: 'Local Egress SIEM',
      mode: 'tls_intercept',
    });

    await probeProfileReachabilityThroughUi(page, options, accessToken, proxyProfile);
    await probeProfileReachabilityThroughUi(page, options, accessToken, tlsProfile);

    const clonedProfile = await cloneAndDisableProfile(page, options, accessToken, tlsProfile);
    await verifyDisabledProfileIsNotHealthyLaunchChoice(page, options, clonedProfile.id);
    sessionIds = await verifySessionEffectiveEgress(accessToken, options, {
      proxyProfile,
      tlsProfile,
      observerSince,
      runLabel,
    });

    console.log(JSON.stringify({
      proxyProfileId: proxyProfile.id,
      tlsProfileId: tlsProfile.id,
      disabledProfileId: clonedProfile.id,
      sessionIds,
    }, null, 2));
  } finally {
    const accessToken = await getAdminAccessToken(page).catch(() => '');
    if (accessToken) {
      for (const sessionId of sessionIds) {
        await deleteSession(accessToken, options, sessionId).catch((error) => {
          log(`Session cleanup for ${sessionId} failed: ${error.message}`);
        });
      }
    }
    await context.close();
    await browser.close();
  }
}

async function probeProfileReachabilityThroughUi(page, options, accessToken, profile) {
  await page.getByTestId('egress-profile-search').fill(profile.name);
  await page.locator(`[data-testid="egress-profile-row"][data-profile-id="${profile.id}"]`).click();
  await page.getByTestId('egress-profile-reachability-probe').click();
  const diagnostics = await poll(
    `profile reachability probe ${profile.name}`,
    async () => {
      const refreshed = await fetchJson(`${apiOrigin(options)}/api/v1/egress-profiles/${profile.id}/diagnostics`, {
        headers: { Authorization: `Bearer ${accessToken}` },
      });
      return refreshed?.proof ?? null;
    },
    (proof) => proof?.profile_reachability_collected === true && proof?.profile_reachability_healthy === true,
    options.connectTimeoutMs,
    250,
  );
  if (!diagnostics.profile_reachability_healthy) {
    throw new Error(`Expected profile reachability diagnostics to pass for ${profile.name}, got ${JSON.stringify(diagnostics)}`);
  }
}

async function createProfileThroughUi(page, options, profile) {
  await page.getByTestId('egress-profile-new').click();
  await page.getByTestId('egress-profile-name').fill(profile.name);
  await page.getByTestId('egress-profile-description').fill(profile.description);
  await page.getByTestId('egress-profile-labels').fill(profile.labels);
  await page.getByTestId('egress-profile-proxy-url').fill(profile.proxyUrl);
  await page.getByTestId('egress-profile-bypass-rules').fill(profile.bypassRules);
  await page.getByTestId('egress-profile-observation-mode').selectOption(profile.mode);
  if (profile.customCaRef) {
    await page.getByTestId('egress-profile-custom-ca-ref').fill(profile.customCaRef);
  }
  if (profile.customCaName) {
    await page.getByLabel('Custom CA name').fill(profile.customCaName);
  }
  if (profile.logSinkRef) {
    await page.getByTestId('egress-profile-log-sink-ref').fill(profile.logSinkRef);
  }
  if (profile.logSinkName) {
    await page.getByLabel('Log-sink name').fill(profile.logSinkName);
  }
  await page.getByTestId('egress-profile-save').click();
  await page.getByTestId('egress-profile-search').fill(profile.name);
  await poll(
    `egress profile row ${profile.name}`,
    async () => await page.getByTestId('egress-profile-row').filter({ hasText: profile.name }).count(),
    (count) => count > 0,
    options.connectTimeoutMs,
    100,
  );
  await page.locator(`[data-testid="egress-profile-row"]`).filter({ hasText: profile.name }).first().click();
  await page.getByTestId('egress-profile-health').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await page.getByTestId('egress-profile-diagnostics-proof').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  return await fetchProfileByName(page, options, profile.name);
}

async function cloneAndDisableProfile(page, options, accessToken, sourceProfile) {
  const cloneName = `${sourceProfile.name}-disabled`;
  await page.getByTestId('egress-profile-search').fill(sourceProfile.name);
  await page.locator(`[data-testid="egress-profile-row"][data-profile-id="${sourceProfile.id}"]`).click();
  await page.getByTestId('egress-profile-clone').click();
  await page.getByTestId('egress-profile-name').fill(cloneName);
  await page.getByTestId('egress-profile-save').click();
  const clone = await poll(
    'cloned egress profile API resource',
    async () => await fetchProfileByName(page, options, cloneName).catch(() => null),
    Boolean,
    options.connectTimeoutMs,
    100,
  );
  await page.getByTestId('egress-profile-search').fill(cloneName);
  await page.locator(`[data-testid="egress-profile-row"][data-profile-id="${clone.id}"]`).click();
  await page.getByTestId('egress-profile-disable').click();
  return await poll(
    'disabled egress profile state',
    async () => await fetchJson(`${apiOrigin(options)}/api/v1/egress-profiles/${clone.id}`, {
      headers: { Authorization: `Bearer ${accessToken}` },
    }),
    (profile) => profile?.state === 'disabled',
    options.connectTimeoutMs,
    100,
  );
}

async function verifyDisabledProfileIsNotHealthyLaunchChoice(page, options, profileId) {
  await openAdminTab(page, 'sessions');
  await page.getByTestId('session-create-configurator').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  const disabled = await page
    .getByTestId('session-create-egress-profile')
    .locator(`option[value="${profileId}"]`)
    .evaluate((option) => option.disabled);
  if (!disabled) {
    throw new Error(`Expected disabled egress profile ${profileId} to be disabled in session create selector.`);
  }
}

async function verifySessionEffectiveEgress(accessToken, options, context) {
  const { proxyProfile, tlsProfile, observerSince, runLabel } = context;
  const verifiedSessionIds = [];
  const noEgress = await createSession(accessToken, options, {});
  verifiedSessionIds.push(noEgress.id);
  if (noEgress.effective_egress?.profile_id !== null) {
    throw new Error(`Expected no-egress session to have no profile id, got ${JSON.stringify(noEgress.effective_egress)}`);
  }
  if (noEgress.egress_diagnostics?.health !== 'ready') {
    throw new Error(`Expected no-egress diagnostics to be ready, got ${JSON.stringify(noEgress.egress_diagnostics)}`);
  }
  const noEgressDiagnostics = await launchAndFetchEgressDiagnostics(accessToken, options, noEgress.id);
  if (noEgressDiagnostics.profile_id !== null || noEgressDiagnostics.proof?.runtime_launch_observed !== true) {
    throw new Error(`Expected no-egress runtime diagnostics without profile metadata, got ${JSON.stringify(noEgressDiagnostics)}`);
  }
  const noEgressMetadata = dockerSessionMetadata(noEgress.id);
  if (noEgressMetadata.labels['browserpane.egress_profile_id']) {
    throw new Error(`Expected no-egress container to omit egress profile labels, got ${JSON.stringify(noEgressMetadata.labels)}`);
  }
  await deleteSession(accessToken, options, noEgress.id);

  const proxy = await createSession(accessToken, options, {
    network_identity: { egress_profile_id: proxyProfile.id },
  });
  verifiedSessionIds.push(proxy.id);
  if (proxy.effective_egress?.profile_id !== proxyProfile.id || proxy.effective_egress?.tls_interception_enabled) {
    throw new Error(`Expected proxy session effective egress for ${proxyProfile.id}, got ${JSON.stringify(proxy.effective_egress)}`);
  }
  const proxyDiagnostics = await launchAndFetchEgressDiagnostics(accessToken, options, proxy.id);
  if (proxyDiagnostics.health !== 'ready' || proxyDiagnostics.proof_level !== 'runtime_launch_metadata') {
    throw new Error(`Expected proxy runtime diagnostics to be ready with launch metadata, got ${JSON.stringify(proxyDiagnostics)}`);
  }
  const proxyProbe = await runSessionEgressProbe(accessToken, options, proxy.id, runLabel).catch(() => null);
  await assertObserverCorrelation({
    sessionId: proxy.id,
    profileId: proxyProfile.id,
    observerContainer: 'bpane-egress-observer',
    observerSince,
    expectedMode: 'metadata_only',
    expectedHost: probeObserved(proxyProbe) ? 'example.com' : null,
  });
  await deleteSession(accessToken, options, proxy.id);

  const tls = await createSession(accessToken, options, {
    network_identity: { egress_profile_id: tlsProfile.id },
  });
  verifiedSessionIds.push(tls.id);
  if (tls.effective_egress?.profile_id !== tlsProfile.id || !tls.effective_egress?.tls_interception_enabled) {
    throw new Error(`Expected TLS session effective egress for ${tlsProfile.id}, got ${JSON.stringify(tls.effective_egress)}`);
  }
  const tlsDiagnostics = await launchAndFetchEgressDiagnostics(accessToken, options, tls.id);
  if (
    tlsDiagnostics.health !== 'ready'
    || tlsDiagnostics.proof_level !== 'runtime_launch_metadata'
    || !tlsDiagnostics.proof?.custom_ca_launch_config_expected
  ) {
    throw new Error(`Expected TLS runtime diagnostics with custom CA launch proof, got ${JSON.stringify(tlsDiagnostics)}`);
  }
  const tlsProbe = await runSessionEgressProbe(accessToken, options, tls.id, runLabel).catch(() => null);
  await assertObserverCorrelation({
    sessionId: tls.id,
    profileId: tlsProfile.id,
    observerContainer: 'bpane-egress-tls-observer',
    observerSince,
    expectedMode: 'tls_intercept',
    expectedHost: probeObserved(tlsProbe) ? 'example.com' : null,
  });
  await deleteSession(accessToken, options, tls.id);
  return verifiedSessionIds;
}

async function launchAndFetchEgressDiagnostics(accessToken, options, sessionId) {
  await fetchJson(`${apiOrigin(options)}/api/v1/sessions/${sessionId}/automation-access`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  return await fetchJson(`${apiOrigin(options)}/api/v1/sessions/${sessionId}/egress-diagnostics`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function runSessionEgressProbe(accessToken, options, sessionId, runLabel) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/sessions/${sessionId}/egress-diagnostics`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({
      public_ip_url: `https://example.com/?bpane_egress_smoke=${encodeURIComponent(runLabel)}`,
      tls_probe_url: `https://example.com/?bpane_egress_smoke=${encodeURIComponent(runLabel)}`,
      timeout_ms: 10000,
    }),
  });
}

function probeObserved(probe) {
  return probe?.proof?.active_probe_collected === true && probe?.proof?.active_probe_healthy === true;
}

async function assertObserverCorrelation({
  sessionId,
  profileId,
  observerContainer,
  observerSince,
  expectedMode,
  expectedHost,
}) {
  const metadata = dockerSessionMetadata(sessionId);
  if (metadata.labels['browserpane.session_id'] !== sessionId) {
    throw new Error(`Container labels did not include expected session id ${sessionId}: ${JSON.stringify(metadata.labels)}`);
  }
  if (metadata.labels['browserpane.egress_profile_id'] !== profileId) {
    throw new Error(`Container labels did not include expected egress profile ${profileId}: ${JSON.stringify(metadata.labels)}`);
  }
  if (metadata.labels['browserpane.egress_observation_mode'] !== expectedMode) {
    throw new Error(`Container labels did not include expected egress mode ${expectedMode}: ${JSON.stringify(metadata.labels)}`);
  }
  if (!metadata.ipAddress) {
    throw new Error(`Could not resolve Docker network IP for session ${sessionId}.`);
  }

  await poll(
    `${observerContainer} observer log correlation for ${sessionId}`,
    async () => dockerLogs(observerContainer, observerSince),
    (logs) => logs.includes(metadata.ipAddress) && (!expectedHost || logs.includes(expectedHost)),
    15000,
    500,
  );
}

function dockerSessionMetadata(sessionId) {
  const ids = dockerCapture(['ps', '-q', '--filter', `label=browserpane.session_id=${sessionId}`]).trim().split(/\s+/).filter(Boolean);
  if (ids.length !== 1) {
    throw new Error(`Expected exactly one runtime container for session ${sessionId}, got ${ids.length}.`);
  }
  const inspect = JSON.parse(dockerCapture(['inspect', ids[0]]));
  const container = inspect[0];
  const labels = container?.Config?.Labels ?? {};
  const networks = container?.NetworkSettings?.Networks ?? {};
  const ipAddress = Object.values(networks).map((network) => network?.IPAddress).find(Boolean) ?? '';
  return { id: ids[0], labels, ipAddress };
}

function dockerLogs(containerName, since) {
  return dockerCapture(['logs', '--since', since, containerName]);
}

function dockerCapture(args) {
  const result = spawnSync('docker', args, {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  const combined = `${result.stdout ?? ''}${result.stderr ?? ''}`;
  if (result.status !== 0) {
    throw new Error(`docker ${args.join(' ')} failed: ${combined.trim()}`);
  }
  return combined;
}

async function createSession(accessToken, options, body) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/sessions`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}`, 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
}

async function fetchProfileByName(page, options, name) {
  const accessToken = await getAdminAccessToken(page);
  const list = await fetchJson(`${apiOrigin(options)}/api/v1/egress-profiles`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  const profile = list.profiles?.find((candidate) => candidate.name === name);
  if (!profile) {
    throw new Error(`Egress profile ${name} not found.`);
  }
  return profile;
}

run().catch((error) => {
  console.error(`[admin-egress-profiles-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
