import type {
  SessionRecordingPlaybackResource,
  SessionRecordingResource,
} from '../api/recording-types';
import type { LiveBrowserSessionConnection } from '../session/browser-session-types';
import {
  formatRecordingBytes,
  formatRecordingDuration,
  formatRecordingReason,
  formatRecordingTimestamp,
  shortRecordingId,
} from './recording-format';

export type RecordingSegmentCardViewModel = {
  readonly id: string;
  readonly title: string;
  readonly state: string;
  readonly metadata: readonly string[];
  readonly error: string | null;
  readonly canDownload: boolean;
  readonly downloadLabel: string;
};

export type RecordingViewModel = {
  readonly status: string;
  readonly sessionLabel: string;
  readonly artifactLabel: string;
  readonly note: string;
  readonly canStart: boolean;
  readonly canStop: boolean;
  readonly canDownload: boolean;
  readonly busy: boolean;
  readonly error: string | null;
  readonly libraryNote: string;
  readonly libraryStatus: string;
  readonly playbackStatus: string;
  readonly refreshLibraryLabel: string;
  readonly playbackDownloadLabel: string;
  readonly canRefreshLibrary: boolean;
  readonly canDownloadPlaybackExport: boolean;
  readonly segments: readonly RecordingSegmentCardViewModel[];
  readonly emptyLibraryLabel: string;
};

export type RecordingViewModelInput = {
  readonly liveConnection: LiveBrowserSessionConnection | null;
  readonly selectedSessionId: string | null;
  readonly recording: boolean;
  readonly busy: boolean;
  readonly lastArtifactName: string | null;
  readonly error: string | null;
  readonly recordings: readonly SessionRecordingResource[];
  readonly playback: SessionRecordingPlaybackResource | null;
  readonly libraryLoading: boolean;
  readonly libraryLoaded: boolean;
  readonly libraryError: string | null;
  readonly downloadingRecordingId: string | null;
  readonly downloadingPlayback: boolean;
};

export class RecordingViewModelBuilder {
  static build(input: RecordingViewModelInput): RecordingViewModel {
    const handle = input.liveConnection?.handle;
    const supported = Boolean(handle?.startRecording && handle.stopRecording);
    const catalogBusy = input.libraryLoading
      || Boolean(input.downloadingRecordingId)
      || input.downloadingPlayback;
    const selected = Boolean(input.selectedSessionId);
    return {
      status: input.recording ? 'recording' : 'idle',
      sessionLabel: input.selectedSessionId ?? input.liveConnection?.sessionId ?? '--',
      artifactLabel: input.lastArtifactName ?? '--',
      note: supported
        ? 'Records the composed browser view as a local WebM artifact.'
        : 'Connect to a session with recording-capable browser handle support.',
      canStart: Boolean(input.liveConnection) && supported && !input.recording && !input.busy,
      canStop: Boolean(input.liveConnection) && supported && input.recording && !input.busy,
      canDownload: Boolean(input.lastArtifactName) && !input.busy,
      busy: input.busy,
      error: input.error,
      libraryNote: libraryNote(input),
      libraryStatus: libraryStatus(input),
      playbackStatus: playbackStatus(input.playback),
      refreshLibraryLabel: input.libraryLoading ? 'Loading...' : 'Refresh library',
      playbackDownloadLabel: input.downloadingPlayback ? 'Downloading export...' : 'Download session export',
      canRefreshLibrary: selected && !catalogBusy,
      canDownloadPlaybackExport: selected && !catalogBusy
        && (input.playback?.included_segment_count ?? 0) > 0,
      segments: input.recordings.map((recording) => segment(recording, catalogBusy, input.downloadingRecordingId)),
      emptyLibraryLabel: input.libraryLoading
        ? 'Loading retained recordings...'
        : 'No retained recording segments are available for this session yet.',
    };
  }
}

function segment(
  recording: SessionRecordingResource,
  catalogBusy: boolean,
  downloadingRecordingId: string | null,
): RecordingSegmentCardViewModel {
  const downloading = downloadingRecordingId === recording.id;
  return {
    id: recording.id,
    title: `Segment ${shortRecordingId(recording.id)}`,
    state: recording.state,
    metadata: [
      `Started: ${formatRecordingTimestamp(recording.started_at)}`,
      `Completed: ${formatRecordingTimestamp(recording.completed_at)}`,
      `Duration: ${formatRecordingDuration(recording.duration_ms)}`,
      `Size: ${formatRecordingBytes(recording.bytes)}`,
      `Termination: ${formatRecordingReason(recording.termination_reason)}`,
    ],
    error: recording.error,
    canDownload: recording.artifact_available && !catalogBusy,
    downloadLabel: downloadLabel(recording, downloading),
  };
}

function downloadLabel(recording: SessionRecordingResource, downloading: boolean): string {
  if (downloading) {
    return 'Downloading...';
  }
  if (!recording.artifact_available) {
    return recording.state === 'ready' ? 'Artifact unavailable' : 'Artifact pending';
  }
  return 'Download segment';
}

function libraryStatus(input: RecordingViewModelInput): string {
  if (input.libraryLoading) {
    return 'Library: loading...';
  }
  if (!input.libraryLoaded) {
    return 'Library: idle';
  }
  const count = input.recordings.length;
  return `Library: ${count} segment${count === 1 ? '' : 's'}`;
}

function playbackStatus(playback: SessionRecordingPlaybackResource | null): string {
  if (!playback) {
    return 'Playback export: --';
  }
  return `Playback export: ${playback.state} · ${playback.included_segment_count}/${playback.segment_count} segments · ${formatRecordingBytes(playback.included_bytes)}`;
}

function libraryNote(input: RecordingViewModelInput): string {
  if (!input.selectedSessionId) {
    return 'Select a session to inspect persisted recording segments and the session export bundle.';
  }
  if (input.libraryLoading) {
    return `Loading retained recordings for session ${shortRecordingId(input.selectedSessionId)}...`;
  }
  if (input.libraryError) {
    return `Recording library failed to load: ${input.libraryError}`;
  }
  if (!input.libraryLoaded) {
    return 'Refresh the library to inspect retained recording segments and the session export bundle.';
  }
  if (input.recordings.length === 0) {
    return 'No retained recording segments exist for the selected session yet.';
  }
  return `Loaded ${input.recordings.length} retained recording segment${input.recordings.length === 1 ? '' : 's'} for session ${shortRecordingId(input.selectedSessionId)}.`;
}
