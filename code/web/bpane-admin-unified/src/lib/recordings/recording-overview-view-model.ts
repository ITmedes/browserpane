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
  readonly canDownload: boolean;
  readonly downloadKind: 'recording_segment' | 'playback_export' | 'unavailable';
  readonly downloadDescription: string;
  readonly downloadFileName: string;
  readonly badges: readonly string[];
};

export type RecordingOverviewModel = {
  readonly metrics: readonly RecordingOverviewMetric[];
  readonly rows: readonly RecordingOverviewRow[];
};

export function buildRecordingOverviewModel(
  entries: readonly RecordingCatalogEntry[],
): RecordingOverviewModel {
  const downloadableSegmentCounts = downloadableSegmentCountBySession(entries);
  return {
    metrics: [
      metric('total', 'Recordings', entries.length),
      metric('ready', 'Downloadable', entries.filter((entry) => isDownloadableRecording(entry.recording)).length),
      metric('active', 'Active segments', entries.filter((entry) => isActiveRecording(entry.recording)).length),
      metric('failed', 'Failed', entries.filter((entry) => entry.recording.state === 'failed').length),
    ],
    rows: entries.map((entry) => recordingOverviewRow(entry, downloadableSegmentCounts[entry.session.id] ?? 0)),
  };
}

export function recordingOverviewRow(
  entry: RecordingCatalogEntry,
  downloadableSessionSegmentCount = 1,
): RecordingOverviewRow {
  const { recording, session } = entry;
  const shortId = shortIdentifier(recording.id);
  const shortSessionId = shortIdentifier(session.id);
  const downloadable = isDownloadableRecording(recording);
  const multiSegmentDownload = downloadable && downloadableSessionSegmentCount > 1;
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
    canDownload: downloadable,
    downloadKind: downloadable
      ? multiSegmentDownload
        ? 'playback_export'
        : 'recording_segment'
      : 'unavailable',
    downloadDescription: downloadable
      ? multiSegmentDownload
        ? 'Download session playback ZIP'
        : 'Download recording WebM'
      : 'Recording artifact is unavailable',
    downloadFileName: multiSegmentDownload
      ? `bpane-recording-playback-${shortSessionId}.zip`
      : `bpane-recording-${shortSessionId}-${shortId}.${recordingExtension(format)}`,
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

export function isDownloadableRecording(recording: SessionRecordingResource): boolean {
  return recording.state === 'ready' && recording.artifact_available;
}

function downloadableSegmentCountBySession(
  entries: readonly RecordingCatalogEntry[],
): Readonly<Record<string, number>> {
  const counts: Record<string, number> = {};
  for (const entry of entries) {
    if (isDownloadableRecording(entry.recording)) {
      counts[entry.session.id] = (counts[entry.session.id] ?? 0) + 1;
    }
  }
  return counts;
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
