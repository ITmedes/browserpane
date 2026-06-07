import assert from 'node:assert/strict';
import test from 'node:test';
import {
  collectUsageReports,
  parseArgs,
  parseSquidAccessLine,
  runtimeIpMapFromDockerInspect,
  runReporter,
  sanitizeObserverId,
} from './egress-usage-reporter.mjs';

test('parses native Squid access lines without retaining URL or status fields', () => {
  const parsed = parseSquidAccessLine(
    '2026-06-05T13:50:24.000000000Z 1817541024.123 42 172.28.0.44 TCP_TUNNEL/200 8192 CONNECT example.com:443 - HIER_DIRECT/93.184.216.34 -',
  );

  assert.deepEqual(parsed, {
    clientIp: '172.28.0.44',
    observedAt: '2027-08-06T08:30:24.123Z',
    observedAtMs: 1817541024123,
    rxBytes: 8192,
    txBytes: 0,
  });
  assert.equal(Object.hasOwn(parsed, 'url'), false);
  assert.equal(Object.hasOwn(parsed, 'status'), false);
});

test('maps Docker runtime container IPs to BrowserPane session labels', () => {
  const runtimeIpMap = runtimeIpMapFromDockerInspect([
    {
      Name: '/bpane-runtime-a',
      Config: {
        Labels: {
          'browserpane.session_id': 'session-a',
          'browserpane.egress_profile_id': 'profile-a',
        },
      },
      NetworkSettings: {
        Networks: {
          deploy_bpane_internal: { IPAddress: '172.28.0.44' },
        },
      },
    },
  ]);

  assert.deepEqual(runtimeIpMap.get('172.28.0.44'), {
    sessionId: 'session-a',
    egressProfileId: 'profile-a',
    containerName: 'bpane-runtime-a',
  });
});

test('aggregates only correlated Squid byte counters into sanitized reports', () => {
  const runtimeIpMap = new Map([
    ['172.28.0.44', { sessionId: 'session-a', egressProfileId: 'profile-a' }],
  ]);

  const { reports, nextState } = collectUsageReports({
    runtimeIpMap,
    linesByObserver: {
      'local-squid': [
        '1817541024.123 42 172.28.0.44 TCP_TUNNEL/200 8192 CONNECT example.com:443 - HIER_DIRECT/93.184.216.34 -',
        '1817541025.123 12 172.28.0.99 TCP_MISS/200 4096 GET http://not-a-runtime.example/ - HIER_DIRECT/93.184.216.34 text/html',
        '1817541026.123 8 172.28.0.44 TCP_MISS/200 1024 GET http://example.com/ - HIER_DIRECT/93.184.216.34 text/html',
        '1817541027.123 9 172.28.0.99 TCP_MISS/200 2048 GET http://not-a-runtime.example/ - HIER_DIRECT/93.184.216.34 text/html',
      ],
    },
    previousState: { containers: {} },
  });

  assert.deepEqual(reports, [{
    session_id: 'session-a',
    egress_profile_id: 'profile-a',
    observer_id: 'local-squid',
    source_kind: 'proxy',
    rx_bytes_delta: 9216,
    tx_bytes_delta: 0,
    observed_at: '2027-08-06T08:30:26.123Z',
  }]);
  assert.equal(nextState.containers['local-squid'].lastTimestampMs, 1817541027123);
});

test('state watermark prevents duplicate reports for already seen log lines', () => {
  const runtimeIpMap = new Map([
    ['172.28.0.44', { sessionId: 'session-a', egressProfileId: null }],
  ]);
  const linesByObserver = {
    'local-squid': [
      '1817541024.123 42 172.28.0.44 TCP_TUNNEL/200 8192 CONNECT example.com:443 - HIER_DIRECT/93.184.216.34 -',
    ],
  };

  const first = collectUsageReports({ runtimeIpMap, linesByObserver, previousState: { containers: {} } });
  const second = collectUsageReports({ runtimeIpMap, linesByObserver, previousState: first.nextState });

  assert.equal(first.reports.length, 1);
  assert.deepEqual(second.reports, []);
});

test('runReporter posts only sanitized egress usage report fields', async () => {
  const posted = [];
  const result = await runReporter({
    apiUrl: 'http://localhost:8080',
    accessToken: 'owner-token',
    containers: ['bpane-egress-observer'],
    dryRun: false,
    since: '10m',
    statePath: '/tmp/reporter-state.json',
    sourceKind: 'proxy',
  }, {
    docker: {
      inspectBrowserPaneRuntimes: () => [{
        Name: '/bpane-runtime-a',
        Config: { Labels: { 'browserpane.session_id': 'session-a' } },
        NetworkSettings: { Networks: { network: { IPAddress: '172.28.0.44' } } },
      }],
      logs: () => [
        '1817541024.123 42 172.28.0.44 TCP_TUNNEL/200 8192 CONNECT example.com:443 - HIER_DIRECT/93.184.216.34 -',
      ],
    },
    readState: async () => ({ containers: {} }),
    writeState: async () => {},
    postReport: async (_options, report) => posted.push(report),
    log: () => {},
  });

  assert.equal(result.reports.length, 1);
  assert.deepEqual(posted, [{
    session_id: 'session-a',
    egress_profile_id: null,
    observer_id: 'bpane-egress-observer',
    source_kind: 'proxy',
    rx_bytes_delta: 8192,
    tx_bytes_delta: 0,
    observed_at: '2027-08-06T08:30:24.123Z',
  }]);
});

test('dry-run previews sanitized reports without advancing persisted state', async () => {
  let wroteState = false;
  const logs = [];
  const result = await runReporter({
    apiUrl: 'http://localhost:8080',
    accessToken: '',
    containers: ['bpane-egress-observer'],
    dryRun: true,
    since: '10m',
    statePath: '/tmp/reporter-state.json',
    sourceKind: 'proxy',
  }, {
    docker: {
      inspectBrowserPaneRuntimes: () => [{
        Name: '/bpane-runtime-a',
        Config: { Labels: { 'browserpane.session_id': 'session-a' } },
        NetworkSettings: { Networks: { network: { IPAddress: '172.28.0.44' } } },
      }],
      logs: () => [
        '1817541024.123 42 172.28.0.44 TCP_TUNNEL/200 8192 CONNECT example.com:443 - HIER_DIRECT/93.184.216.34 -',
      ],
    },
    readState: async () => ({ containers: {} }),
    writeState: async () => {
      wroteState = true;
    },
    postReport: async () => {
      throw new Error('dry-run must not call the API');
    },
    log: (line) => logs.push(line),
  });

  assert.equal(result.reports.length, 1);
  assert.equal(wroteState, false);
  assert.match(logs[0], /"session_id":"session-a"/u);
  assert.doesNotMatch(logs[0], /example\.com/u);
});

test('rejects observer identifiers that the gateway would reject', () => {
  assert.equal(sanitizeObserverId('bpane-egress-observer'), 'bpane-egress-observer');
  assert.throws(() => sanitizeObserverId('https://proxy.example'), /not compatible/iu);
});

test('parseArgs keeps defaults local and accepts repeatable proxy containers', () => {
  assert.deepEqual(parseArgs(['--dry-run', '--container', 'a', '--container', 'b', '--since', '5m'], {}), {
    apiUrl: 'http://localhost:8080',
    accessToken: '',
    containers: ['a', 'b'],
    dryRun: true,
    since: '5m',
    statePath: '.bpane-egress-usage-reporter-state.json',
    sourceKind: 'proxy',
  });
});

test('parseArgs rejects source kind values outside the gateway contract', () => {
  assert.throws(
    () => parseArgs(['--source-kind', 'forward_proxy'], {}),
    /Invalid source kind/iu,
  );
});
