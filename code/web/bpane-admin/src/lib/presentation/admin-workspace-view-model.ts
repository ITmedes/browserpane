export type AdminFeaturePanelId =
  | 'sessions'
  | 'lifecycle'
  | 'display'
  | 'files'
  | 'policy'
  | 'workflows'
  | 'recording'
  | 'metrics'
  | 'logs';

export type AdminFeaturePanelViewModel = {
  readonly id: AdminFeaturePanelId;
  readonly label: string;
  readonly title: string;
  readonly description: string;
  readonly status: string;
  readonly implemented: boolean;
  readonly controls: readonly string[];
  readonly metrics: readonly string[];
};

export type BrowserStageViewModel = {
  readonly status: string;
  readonly sessionLabel: string;
  readonly connectionLabel: string;
};

export type AdminWorkspaceViewModel = {
  readonly browser: BrowserStageViewModel;
  readonly panels: readonly AdminFeaturePanelViewModel[];
};

export class AdminWorkspaceViewModelBuilder {
  static build(input: {
    readonly browserStatus: string;
    readonly selectedSessionId: string | null;
    readonly sessionCount: number;
    readonly fileCount: number;
    readonly connected: boolean;
  }): AdminWorkspaceViewModel {
    return {
      browser: {
        status: input.browserStatus,
        sessionLabel: input.selectedSessionId ? shortId(input.selectedSessionId) : 'no session',
        connectionLabel: input.connected ? 'connected' : 'not connected',
      },
      panels: [
        panel('sessions', 'Sessions', 'Login, start, reconnect, share', 'Owner-scoped session entrypoint.', `${input.sessionCount} visible`, true, [
          'New session',
          'Join / reconnect',
          'Delegate MCP',
        ], ['visible sessions', 'selected session']),
        panel('lifecycle', 'Lifecycle', 'Runtime state, stop blockers, live clients', 'Inspect lifecycle safety before mutating a runtime.', input.connected ? 'busy' : 'ready', true, [
          'Refresh status',
          'Stop selected',
          'Kill selected',
          'Disconnect all clients',
        ], ['runtime', 'presence', 'connections']),
        panel('display', 'Display', 'Render backend, HiDPI, scroll copy, media controls', 'Session display and browser media controls from the dev harness.', input.connected ? 'live' : 'next connect', true, [
          'Render backend',
          'HiDPI',
          'Scroll copy',
          'Camera / microphone / upload',
        ], ['resolution', 'render backend', 'tile stats']),
        panel('files', 'Files', 'Runtime uploads and downloads', 'Owner-scoped file artifacts recorded for the selected session.', `${input.fileCount} files`, true, [
          'Refresh files',
          'Download file artifact',
        ], ['file count', 'artifact source']),
        panel('policy', 'Policy', 'Local file access guardrails', 'Browser policy visibility and runtime probes.', input.selectedSessionId ? 'visible' : 'select', true, [
          'Inspect effective policy',
          'Run local-file probe',
        ], ['file:// navigation', 'File System Access API']),
        panel('workflows', 'Workflows', 'Invoke, inspect, and download run artifacts', 'Workflow definition, run, intervention, and artifact controls.', 'api pending', true, [
          'Load definition',
          'Invoke run',
          'Operator intervention',
          'Download produced files',
        ], ['run status', 'admission', 'runtime hold']),
        panel('recording', 'Recording', 'Capture and export the composed session output', 'Local browser recording controls.', input.connected ? 'ready' : 'connect', true, [
          'Start recording',
          'Stop and save WebM',
          'Download session export',
        ], ['recording status', 'retained segments']),
        panel('metrics', 'Metrics', 'Benchmark samples and transport health', 'Runtime sampling and transport performance summary.', input.connected ? 'ready' : 'connect', true, [
          'Start sample',
          'Stop sample',
          'Copy metrics',
        ], ['surface throughput', 'tiles', 'video']),
        panel('logs', 'Logs', 'Session and transport event timeline', 'Operator-facing diagnostics without crowding the browser stage.', 'local', true, [
          'Clear logs',
          'Copy diagnostics',
        ], ['auth events', 'transport events']),
      ],
    };
  }
}

function panel(
  id: AdminFeaturePanelId,
  label: string,
  title: string,
  description: string,
  status: string,
  implemented: boolean,
  controls: readonly string[],
  metrics: readonly string[],
): AdminFeaturePanelViewModel {
  return { id, label, title, description, status, implemented, controls, metrics };
}

function shortId(value: string): string {
  return value.length > 13 ? `${value.slice(0, 8)}...${value.slice(-4)}` : value;
}
