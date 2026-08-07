import { formatBytes, formatDateTime, formatDuration, type ProjectTone } from '$lib/projects/project-formatters';
import type { SessionResource } from '$lib/sessions/session-types';
import type {
  SessionRecordingPlaybackResource,
  SessionRecordingResource,
} from './recording-types';
import { isActiveRecording, isDownloadableRecording } from './recording-overview-view-model';

export type SessionRecordingSegmentRow = {
  readonly id: string;
  readonly previousId: string | null;
  readonly state: string;
  readonly stateTone: ProjectTone;
  readonly artifact: string;
  readonly format: string;
  readonly size: string;
  readonly duration: string;
  readonly startedAt: string;
  readonly completedAt: string;
  readonly termination: string;
  readonly error: string | null;
};

export type SessionRecordingDetailModel = {
  readonly policyLabel: string;
  readonly policyEnabled: boolean;
  readonly canEnablePolicy: boolean;
  readonly canDisablePolicy: boolean;
  readonly rows: readonly SessionRecordingSegmentRow[];
  readonly playbackState: string;
  readonly playbackTone: ProjectTone;
  readonly playbackSummary: string;
  readonly includedSize: string;
  readonly includedDuration: string;
  readonly canDownload: boolean;
  readonly downloadKind: 'segment' | 'export' | 'none';
  readonly downloadLabel: string;
  readonly downloadFileName: string;
  readonly downloadableRecording: SessionRecordingResource | null;
};

export function buildSessionRecordingDetailModel(
  session: SessionResource,
  recordings: readonly SessionRecordingResource[],
  playback: SessionRecordingPlaybackResource | null,
): SessionRecordingDetailModel {
  const downloadable = recordings.filter(isDownloadableRecording);
  const includedCount = playback?.included_segment_count ?? downloadable.length;
  const canDownload = includedCount > 0;
  const downloadKind = !canDownload ? 'none' : includedCount === 1 ? 'segment' : 'export';
  const shortSessionId = session.id.length <= 12 ? session.id : session.id.slice(0, 12);
  const policyEnabled = session.recording.mode !== 'disabled';
  const terminal = ['killed', 'cancelled', 'failed'].includes(session.state);
  return {
    policyLabel: policyEnabled
      ? `${session.recording.mode} / ${session.recording.format || 'webm'}`
      : 'disabled',
    policyEnabled,
    canEnablePolicy: !terminal && !policyEnabled,
    canDisablePolicy: !terminal && policyEnabled,
    rows: recordings.map(segmentRow),
    playbackState: playback?.state ?? (recordings.length === 0 ? 'empty' : 'unavailable'),
    playbackTone: playbackTone(playback),
    playbackSummary: playback
      ? `${playback.included_segment_count}/${playback.segment_count} segments included`
      : 'Playback summary unavailable',
    includedSize: playback ? formatBytes(playback.included_bytes) ?? '0 B' : 'Unavailable',
    includedDuration: playback ? formatDuration(playback.included_duration_ms) ?? '0s' : 'Unavailable',
    canDownload,
    downloadKind,
    downloadLabel: downloadKind === 'segment'
      ? 'Download WebM'
      : downloadKind === 'export'
        ? 'Download playback ZIP'
        : 'Recording unavailable',
    downloadFileName: downloadKind === 'export'
      ? `bpane-recording-playback-${shortSessionId}.zip`
      : `bpane-recording-${shortSessionId}.${downloadable[0]?.format === 'webm' ? 'webm' : 'bin'}`,
    downloadableRecording: downloadKind === 'segment' ? downloadable[0] ?? null : null,
  };
}

function segmentRow(recording: SessionRecordingResource): SessionRecordingSegmentRow {
  return {
    id: recording.id,
    previousId: recording.previous_recording_id ?? null,
    state: recording.state,
    stateTone: recording.state === 'ready'
      ? recording.artifact_available ? 'success' : 'warning'
      : recording.state === 'failed' ? 'danger' : isActiveRecording(recording) ? 'warning' : 'neutral',
    artifact: recording.artifact_available ? 'available' : 'unavailable',
    format: recording.format,
    size: formatBytes(recording.bytes) ?? 'Unavailable',
    duration: formatDuration(recording.duration_ms) ?? 'Unavailable',
    startedAt: formatDateTime(recording.started_at),
    completedAt: recording.completed_at ? formatDateTime(recording.completed_at) : 'Not completed',
    termination: recording.termination_reason?.replaceAll('_', ' ') ?? 'Not reported',
    error: recording.error?.trim() || null,
  };
}

function playbackTone(playback: SessionRecordingPlaybackResource | null): ProjectTone {
  if (!playback || playback.state === 'empty') {
    return 'neutral';
  }
  return playback.state === 'ready' ? 'success' : 'warning';
}
