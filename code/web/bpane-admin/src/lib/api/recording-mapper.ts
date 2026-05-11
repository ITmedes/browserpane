import type {
  SessionRecordingListResponse,
  SessionRecordingPlaybackResource,
  SessionRecordingResource,
} from './recording-types';
import { expectBoolean, expectNumber, expectRecord, expectString } from './control-wire';

export class RecordingMapper {
  static toRecordingList(payload: unknown): SessionRecordingListResponse {
    const object = expectRecord(payload, 'recording list response');
    if (!Array.isArray(object.recordings)) {
      throw new Error('recording list response must contain a recordings array');
    }
    return {
      recordings: object.recordings.map((recording) => this.toRecording(recording)),
    };
  }

  static toRecording(payload: unknown): SessionRecordingResource {
    const object = expectRecord(payload, 'recording resource');
    return {
      id: expectString(object.id, 'recording resource id'),
      session_id: expectString(object.session_id, 'recording resource session_id'),
      previous_recording_id: nullableString(object.previous_recording_id, 'previous_recording_id'),
      state: expectString(object.state, 'recording resource state'),
      format: expectString(object.format, 'recording resource format'),
      mime_type: nullableString(object.mime_type, 'recording resource mime_type'),
      bytes: nullableNumber(object.bytes, 'recording resource bytes'),
      duration_ms: nullableNumber(object.duration_ms, 'recording resource duration_ms'),
      error: nullableString(object.error, 'recording resource error'),
      termination_reason: nullableString(object.termination_reason, 'termination_reason'),
      artifact_available: expectBoolean(object.artifact_available, 'artifact_available'),
      content_path: expectString(object.content_path, 'recording resource content_path'),
      started_at: expectString(object.started_at, 'recording resource started_at'),
      completed_at: nullableString(object.completed_at, 'recording resource completed_at'),
      created_at: expectString(object.created_at, 'recording resource created_at'),
      updated_at: expectString(object.updated_at, 'recording resource updated_at'),
    };
  }

  static toPlayback(payload: unknown): SessionRecordingPlaybackResource {
    const object = expectRecord(payload, 'recording playback resource');
    return {
      session_id: expectString(object.session_id, 'recording playback session_id'),
      state: expectString(object.state, 'recording playback state'),
      segment_count: expectNumber(object.segment_count, 'recording playback segment_count'),
      included_segment_count: expectNumber(object.included_segment_count, 'included_segment_count'),
      failed_segment_count: expectNumber(object.failed_segment_count, 'failed_segment_count'),
      active_segment_count: expectNumber(object.active_segment_count, 'active_segment_count'),
      missing_artifact_segment_count: expectNumber(object.missing_artifact_segment_count, 'missing_artifact_segment_count'),
      included_bytes: expectNumber(object.included_bytes, 'recording playback included_bytes'),
      included_duration_ms: expectNumber(object.included_duration_ms, 'included_duration_ms'),
      manifest_path: expectString(object.manifest_path, 'recording playback manifest_path'),
      export_path: expectString(object.export_path, 'recording playback export_path'),
      generated_at: expectString(object.generated_at, 'recording playback generated_at'),
    };
  }
}

function nullableString(value: unknown, label: string): string | null {
  if (value === null || value === undefined) {
    return null;
  }
  return expectString(value, label);
}

function nullableNumber(value: unknown, label: string): number | null {
  if (value === null || value === undefined) {
    return null;
  }
  return expectNumber(value, label);
}
