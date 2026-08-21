import assert from 'node:assert/strict';
import process from 'node:process';
import { chromium } from 'playwright-core';
import { GatewayProtocolSmokeApi } from './gateway-protocol-smoke-api.mjs';
import { GatewayProtocolSmokePeer } from './gateway-protocol-smoke-peer.mjs';
import { GatewayProtocolSmokeWire } from './gateway-protocol-smoke-wire.mjs';
import {
  cleanupWorkflowSmokeSessions,
  configurePage,
  createLogger,
  ensureLoggedIn,
  getAccessToken,
  launchChrome,
  parseSmokeArgs,
  poll,
  recreateComposeServices,
} from './workflow-smoke-lib.mjs';
async function connectCurrentClient(page, options, api, sessionId) {
  await page.evaluate(async (id) => {
    await window.__bpaneControl.refreshSessions({ preserveSelection: true, silent: true });
    await window.__bpaneControl.selectSession(id);
    await window.__bpaneControl.connectSelected({ clientRole: 'interactive' });
  }, sessionId);
  await page.waitForFunction(
    (id) => window.__bpaneControl?.getState?.()?.connected === true
      && window.__bpaneControl?.getState?.()?.sessionId === id,
    sessionId,
    { timeout: options.connectTimeoutMs },
  );
  await api.waitForClients(sessionId, 1);
}
async function disconnectCurrentClient(page, options, api, sessionId) {
  await page.evaluate(async () => {
    if (window.__bpaneControl?.getState?.()?.connected) await window.__bpaneControl.disconnect();
  });
  await page.waitForFunction(
    () => window.__bpaneControl?.getState?.()?.connected !== true,
    null,
    { timeout: options.connectTimeoutMs },
  );
  await api.waitForClients(sessionId, 0);
}
async function waitForGateway(options) {
  await poll('gateway readiness after configuration change', async () => {
    try {
      return (await fetch('http://localhost:8932/readyz', {
        signal: AbortSignal.timeout(5_000),
      })).ok;
    } catch {
      return false;
    }
  }, Boolean, options.connectTimeoutMs, 500);
}

async function expectPreHandshakeReject(api, peer, sessionId, initialBytes, expectedFailure) {
  const ticket = await api.issueTicket(sessionId);
  const frames = await peer.exchange({
    gatewayUrl: ticket.connect.gateway_url,
    ticket: ticket.token,
    initialBytes,
    targetTags: [0x0C],
  });
  GatewayProtocolSmokeWire.assertReject(frames, expectedFailure);
}
async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-gateway-protocol-smoke.mjs');
  const log = createLogger('gateway-protocol-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();
  const sessionIds = [];
  let api;
  let legacyDisabled = false;

  try {
    await configurePage(page, options);
    await ensureLoggedIn(page, options);
    await page.waitForFunction(() => Boolean(window.__bpaneControl), {
      timeout: options.connectTimeoutMs,
    });
    const accessToken = await getAccessToken(page);
    api = new GatewayProtocolSmokeApi(accessToken, options);
    const peer = new GatewayProtocolSmokePeer(page, options.connectTimeoutMs);
    await cleanupWorkflowSmokeSessions(accessToken, options, log);

    const untouched = await api.createSession('pre-handshake-rejections');
    sessionIds.push(untouched.id);
    await expectPreHandshakeReject(
      api, peer, untouched.id, GatewayProtocolSmokeWire.hello([2]), 0x0001,
    );
    await expectPreHandshakeReject(
      api, peer, untouched.id, GatewayProtocolSmokeWire.malformedHello(), 0x0003,
    );
    await expectPreHandshakeReject(
      api, peer, untouched.id, GatewayProtocolSmokeWire.hello([1], [0xFFFF]), 0x0002,
    );
    await expectPreHandshakeReject(
      api, peer, untouched.id, GatewayProtocolSmokeWire.prematureServerSelection(), 0x0007,
    );
    await expectPreHandshakeReject(
      api, peer, untouched.id, GatewayProtocolSmokeWire.oversizedFrameHeader(), 0x0008,
    );
    await expectPreHandshakeReject(api, peer, untouched.id, [], 0x0005);
    const untouchedAfter = await api.getSession(untouched.id);
    assert.equal(untouchedAfter.status.runtime_state, 'not_started');
    assert.equal(untouchedAfter.status.connection_counts.total_clients, 0);

    const active = await api.createSession('selection-and-compatibility');
    sessionIds.push(active.id);
    let ticket = await api.issueTicket(active.id);
    let frames = await peer.exchange({
      gatewayUrl: ticket.connect.gateway_url,
      ticket: ticket.token,
      initialBytes: GatewayProtocolSmokeWire.hello([1], [0x0002], [0x000B, 0x000E]),
      afterSelectionBytes: GatewayProtocolSmokeWire.resolutionRequest(),
      targetTags: [0x0B, 0x03],
    });
    GatewayProtocolSmokeWire.assertSelection(frames, [0x0002, 0x000B, 0x000E]);
    GatewayProtocolSmokeWire.assertSessionReady(frames, 0x02);
    await api.waitForClients(active.id, 0);
    ticket = await api.issueTicket(active.id);
    frames = await peer.exchange({
      gatewayUrl: ticket.connect.gateway_url,
      ticket: ticket.token,
      initialBytes: GatewayProtocolSmokeWire.hello([1], [], [0x0008, 0x000D]),
      afterSelectionBytes: GatewayProtocolSmokeWire.resolutionRequest(1024, 768),
      targetTags: [0x0B, 0x03],
    });
    GatewayProtocolSmokeWire.assertSelection(frames, [0x0008, 0x000D]);
    GatewayProtocolSmokeWire.assertSessionReady(frames, 0x21);
    await api.waitForClients(active.id, 0);

    await connectCurrentClient(page, options, api, active.id);
    ticket = await api.issueTicket(active.id);
    frames = await peer.exchange({
      gatewayUrl: ticket.connect.gateway_url,
      ticket: ticket.token,
      initialBytes: GatewayProtocolSmokeWire.hello([1]),
      afterSelectionBytes: GatewayProtocolSmokeWire.hello([1]),
      targetTags: [0x0B, 0x0C],
    });
    GatewayProtocolSmokeWire.assertReject(frames, 0x0007);
    await api.waitForClients(active.id, 1);
    await disconnectCurrentClient(page, options, api, active.id);

    log('Disabling checked legacy compatibility for refusal coverage.');
    legacyDisabled = true;
    recreateComposeServices(['gateway'], {
      envOverrides: { BPANE_GATEWAY_PROTOCOL_LEGACY_COMPATIBILITY: 'false' },
    });
    await waitForGateway(options);
    ticket = await api.issueTicket(active.id);
    frames = await peer.exchange({
      gatewayUrl: ticket.connect.gateway_url,
      ticket: ticket.token,
      initialBytes: GatewayProtocolSmokeWire.resolutionRequest(),
      targetTags: [0x0C],
    });
    GatewayProtocolSmokeWire.assertReject(frames, 0x0004);
    await api.waitForClients(active.id, 0);

    log('Restoring compatibility and proving the current browser recovers.');
    recreateComposeServices(['gateway'], {
      envOverrides: { BPANE_GATEWAY_PROTOCOL_LEGACY_COMPATIBILITY: 'true' },
    });
    legacyDisabled = false;
    await waitForGateway(options);
    await connectCurrentClient(page, options, api, active.id);
    await disconnectCurrentClient(page, options, api, active.id);
    console.log(JSON.stringify({
      negotiatedVersion: 1,
      selectedCapabilitySubset: true,
      preHandshakeSideEffects: false,
      currentClientCompatibility: true,
      isolatedPostSelectionRejection: true,
      disabledLegacyFailure: 'protocol_downgrade_refused',
      recoveryVerified: true,
    }, null, 2));
  } finally {
    if (legacyDisabled) {
      recreateComposeServices(['gateway'], {
        envOverrides: { BPANE_GATEWAY_PROTOCOL_LEGACY_COMPATIBILITY: 'true' },
      });
      await waitForGateway(options).catch(() => {});
    }
    await page.evaluate(async () => {
      if (window.__bpaneControl?.getState?.()?.connected) await window.__bpaneControl.disconnect();
    }).catch(() => {});
    if (api) {
      for (const sessionId of sessionIds) await api.kill(sessionId).catch(() => {});
    }
    await context.close();
    await browser.close();
  }
}

run().catch((error) => {
  console.error(`[gateway-protocol-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
