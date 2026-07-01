import {
  formatBytes,
  formatDateTime,
  formatDuration,
  type ProjectTone,
} from '$lib/projects/project-formatters';
import type {
  RecordingCatalogEntry,
  RecordingCatalogLoadFailure,
  SessionRecordingResource,
} from './recording-types';

export type RecordingOverviewLoadState =
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly message: string }
  | {
      readonly status: 'ready';
      readonly entries: readonly RecordingCatalogEntry[];
      readonly failures: readonly RecordingCatalogLoadFailure[];
    };

export type RecordingActionState =
  | { readonly status: 'idle' }
  | { readonly status: 'running'; readonly label: string }
  | { readonly status: 'success'; readonly message: string }
  | { readonly status: 'error'; readonly message: string };

export type RecordingOverviewMetric = {
  readonly label: string;
  readonly value: string;
  readonly testId: string;
};

export type RecordingOverviewRow = {
  readonly id: string;
  readonly sessionId: string;
  readonly shortId: string;
  readonly shortSessionId: string;
  readonly sessionLabel: string;
  readonly project: string;
  readonly state: string;
  readonly stateTone: ProjectTone;
  readonly artifactLabel: string;
  readonly artifactTone: ProjectTone;
  readonly format: string;
  readonly mimeType: string;
  readonly size: string;
  readonly duration: string;
  readonly startedAt: string;
  readonly completedAt: string;
  readonly updatedAt: string;
  readonly termination: string;
  readonly error: string;
  readonly contentPath: string;
  readonly playbackExportPath: string;
  readonly canDownload: boolean;
  readonly canExportPlayback: boolean;
  readonly downloadFileName: string;
  readonly playbackFileName: string;
  readonly badges: readonly string[];
};

export type RecordingOverviewModel = {
  readonly metrics: readonly RecordingOverviewMetric[];
  readonly rows: readonly RecordingOverviewRow[];
};

export function buildRecordingOverviewModel(
  entries: readonly RecordingCatalogEntry[],
): RecordingOverviewModel {
  return {
    metrics: [
      metric('total', 'Recordings', entries.length),
      metric('ready', 'Downloadable', entries.filter((entry) => entry.recording.artifact_available).length),
      metric('active', 'Active segments', entries.filter((entry) => isActiveRecording(entry.recording)).length),
      metric('failed', 'Failed', entries.filter((entry) => entry.recording.state === 'failed').length),
    ],
    rows: entries.map(recordingOverviewRow),
  };
}

export function recordingOverviewRow(entry: RecordingCatalogEntry): RecordingOverviewRow {
  const { recording, session } = entry;
  const shortId = shortIdentifier(recording.id);
  const shortSessionId = shortIdentifier(session.id);
  const downloadable = recording.artifact_available && recording.state === 'ready';
  const format = recording.format || 'webm';
  return {
    id: recording.id,
    sessionId: session.id,
    shortId,
    shortSessionId,
    sessionLabel: session.labels.name ?? session.labels.title ?? shortSessionId,
    project: session.project?.name ?? session.project_id ?? 'Owner scoped',
    state: recording.state,
    stateTone: recordingStateTone(recording),
    artifactLabel: recording.artifact_available ? 'artifact ready' : 'artifact unavailable',
    artifactTone: recording.artifact_available ? 'success' : recording.state === 'failed' ? 'danger' : 'warning',
    format,
    mimeType: recording.mime_type ?? mimeTypeForFormat(format),
    size: formatBytes(recording.bytes) ?? 'Size unavailable',
    duration: formatDuration(recording.duration_ms) ?? 'Duration unavailable',
    startedAt: formatDateTime(recording.started_at),
    completedAt: recording.completed_at ? formatDateTime(recording.completed_at) : 'Not completed',
    updatedAt: formatDateTime(recording.updated_at),
    termination: recording.termination_reason?.replaceAll('_', ' ') ?? 'No termination reason',
    error: recording.error?.trim() || 'No error',
    contentPath: recording.content_path,
    playbackExportPath: `/api/v1/sessions/${encodeURIComponent(session.id)}/recording-playback/export`,
    canDownload: downloadable,
    canExportPlayback: downloadable,
    downloadFileName: `bpane-recording-${shortSessionId}-${shortId}.${recordingExtension(format)}`,
    playbackFileName: `bpane-recording-playback-${shortSessionId}.zip`,
    badges: [
      recording.state,
      recording.artifact_available ? 'downloadable' : 'missing artifact',
      session.state,
      session.project?.name ?? session.project_id ?? 'owner scoped',
      recording.termination_reason?.replaceAll('_', ' ') ?? null,
      recording.error ? 'error' : null,
    ].filter((value): value is string => Boolean(value)),
  };
}

export function recordingMatchesSearch(
  row: RecordingOverviewRow,
  normalizedQuery: string,
): boolean {
  if (!normalizedQuery) {
    return true;
  }
  return [
    row.id,
    row.sessionId,
    row.shortId,
    row.shortSessionId,
    row.sessionLabel,
    row.project,
    row.state,
    row.artifactLabel,
    row.format,
    row.mimeType,
    row.size,
    row.duration,
    row.termination,
    row.error,
    ...row.badges,
  ].some((value) => value.toLowerCase().includes(normalizedQuery));
}

export function isActiveRecording(recording: SessionRecordingResource): boolean {
  return ['starting', 'recording', 'finalizing'].includes(recording.state);
}

function recordingStateTone(recording: SessionRecordingResource): ProjectTone {
  if (recording.state === 'ready') {
    return recording.artifact_available ? 'success' : 'warning';
  }
  if (isActiveRecording(recording)) {
    return 'warning';
  }
  if (recording.state === 'failed') {
    return 'danger';
  }
  return 'neutral';
}

function shortIdentifier(value: string): string {
  return value.length <= 12 ? value : value.slice(0, 12);
}

function mimeTypeForFormat(format: string): string {
  return format === 'webm' ? 'video/webm' : 'application/octet-stream';
}

function recordingExtension(format: string): string {
  return format === 'webm' ? 'webm' : 'bin';
}

function metric(key: string, label: string, value: number): RecordingOverviewMetric {
  return {
    label,
    value: String(value),
    testId: `recordings-metric-${key}`,
  };
}
