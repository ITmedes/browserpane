import process from 'node:process';
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

    const clonedProfile = await cloneAndDisableProfile(page, options, accessToken, tlsProfile);
    await verifyDisabledProfileIsNotHealthyLaunchChoice(page, options, clonedProfile.id);
    sessionIds = await verifySessionEffectiveEgress(accessToken, options, proxyProfile, tlsProfile);

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

async function verifySessionEffectiveEgress(accessToken, options, proxyProfile, tlsProfile) {
  const verifiedSessionIds = [];
  const noEgress = await createSession(accessToken, options, {});
  verifiedSessionIds.push(noEgress.id);
  if (noEgress.effective_egress?.profile_id !== null) {
    throw new Error(`Expected no-egress session to have no profile id, got ${JSON.stringify(noEgress.effective_egress)}`);
  }
  await deleteSession(accessToken, options, noEgress.id);

  const proxy = await createSession(accessToken, options, {
    network_identity: { egress_profile_id: proxyProfile.id },
  });
  verifiedSessionIds.push(proxy.id);
  if (proxy.effective_egress?.profile_id !== proxyProfile.id || proxy.effective_egress?.tls_interception_enabled) {
    throw new Error(`Expected proxy session effective egress for ${proxyProfile.id}, got ${JSON.stringify(proxy.effective_egress)}`);
  }
  await deleteSession(accessToken, options, proxy.id);

  const tls = await createSession(accessToken, options, {
    network_identity: { egress_profile_id: tlsProfile.id },
  });
  verifiedSessionIds.push(tls.id);
  if (tls.effective_egress?.profile_id !== tlsProfile.id || !tls.effective_egress?.tls_interception_enabled) {
    throw new Error(`Expected TLS session effective egress for ${tlsProfile.id}, got ${JSON.stringify(tls.effective_egress)}`);
  }
  await deleteSession(accessToken, options, tls.id);
  return verifiedSessionIds;
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
