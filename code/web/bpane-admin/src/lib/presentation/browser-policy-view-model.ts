import type { SessionResource } from '../api/control-types';

export type BrowserPolicySignal = {
  readonly label: string;
  readonly value: string;
  readonly tone: 'ok' | 'warn' | 'neutral';
  readonly testId: string;
};

export type BrowserPolicyViewModel = {
  readonly title: string;
  readonly mode: string;
  readonly runtime: string;
  readonly cdpEndpoint: string;
  readonly note: string;
  readonly probeCommand: string;
  readonly canRefresh: boolean;
  readonly canCopyProbeCommand: boolean;
  readonly signals: readonly BrowserPolicySignal[];
};

export class BrowserPolicyViewModelBuilder {
  static build(session: SessionResource | null): BrowserPolicyViewModel {
    if (!session) {
      return {
        title: 'No session selected',
        mode: 'unknown',
        runtime: 'unknown',
        cdpEndpoint: '--',
        note: 'Select a docker-backed session to inspect the expected managed Chromium policy.',
        probeCommand: '',
        canRefresh: false,
        canCopyProbeCommand: false,
        signals: blockedSignals('unknown', 'neutral'),
      };
    }

    const dockerBacked = isDockerBacked(session);
    const cdpEndpoint = session.runtime.cdp_endpoint ?? '';
    return {
      title: dockerBacked ? 'Managed Chromium policy' : 'Runtime policy not guaranteed',
      mode: dockerBacked ? 'deny_all' : 'unknown',
      runtime: `${session.runtime.binding} / ${session.runtime.compatibility_mode}`,
      cdpEndpoint: cdpEndpoint || '--',
      note: dockerBacked
        ? 'Host startup validates file URL and File System Access API deny policies for docker-backed runtimes.'
        : 'This runtime shape does not prove the managed Chromium local-file policy.',
      probeCommand: cdpEndpoint ? buildProbeCommand(cdpEndpoint) : '',
      canRefresh: true,
      canCopyProbeCommand: Boolean(cdpEndpoint),
      signals: dockerBacked ? blockedSignals('blocked', 'ok') : blockedSignals('unknown', 'neutral'),
    };
  }
}

function blockedSignals(value: string, tone: BrowserPolicySignal['tone']): readonly BrowserPolicySignal[] {
  return [
    { label: 'file:// navigation', value, tone, testId: 'policy-file-url' },
    { label: 'File System read', value, tone, testId: 'policy-fs-read' },
    { label: 'File System write', value, tone, testId: 'policy-fs-write' },
  ];
}

function isDockerBacked(session: SessionResource): boolean {
  return session.runtime.binding.includes('docker') || session.connect.compatibility_mode.includes('pool');
}

function buildProbeCommand(cdpEndpoint: string): string {
  return [
    'docker run --rm --network deploy_bpane-internal',
    '-v "$PWD:/workspace:ro"',
    '-w /workspace/code/web/bpane-client',
    'node:22-slim node scripts/cdp-local-file-policy-probe.mjs',
    `--cdp-endpoint ${JSON.stringify(cdpEndpoint)}`,
  ].join(' ');
}
