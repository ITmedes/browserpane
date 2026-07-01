import { describe, expect, it } from 'vitest';

import { sessionResource, sessionStatus } from '$lib/test-utils/session-fixtures';
import { buildSessionDetailModel } from './session-detail-view-model';

describe('buildSessionDetailModel', () => {
  it('renders live metadata and protects stop/release while clients are connected', () => {
    const model = buildSessionDetailModel(
      sessionResource({ totalClients: 1, stopAllowed: false }),
      sessionStatus({ totalClients: 1, stopAllowed: false }),
    );

    expect(model.subtitle).toContain('Support');
    expect(model.sections.map((section) => section.testId)).toContain('session-detail-egress');
    expect(model.connections).toEqual([{ id: 7, role: 'browser-owner' }]);
    expect(model.actions).toMatchObject({
      canDisconnectAll: true,
      canEnableRecording: true,
      canDisableRecording: false,
      canRelease: false,
      canStop: false,
      canKill: true,
    });
  });

  it('enables queue cancellation and hides destructive runtime actions for queued sessions', () => {
    const model = buildSessionDetailModel(
      sessionResource({ state: 'queued', queued: true, totalClients: 0 }),
      sessionStatus({ state: 'queued', totalClients: 0 }),
    );

    expect(model.sections.map((section) => section.testId)).toContain('session-detail-queue');
    expect(model.actions).toMatchObject({
      canCancelQueue: true,
      canDisconnectAll: false,
      canRelease: false,
      canStop: false,
      canKill: false,
    });
  });

  it('allows stop and release for recorder-only sessions', () => {
    const model = buildSessionDetailModel(
      sessionResource({ totalClients: 1, recorderClients: 1, stopAllowed: true, presenceState: 'recording_only' }),
      sessionStatus({ totalClients: 1, recorderClients: 1, stopAllowed: true, presenceState: 'recording_only' }),
    );

    expect(model.actions).toMatchObject({
      canDisconnectAll: false,
      canRelease: true,
      canStop: true,
    });
  });

  it('allows stopped and released sessions to start through the preview connect action', () => {
    for (const state of ['stopped', 'released']) {
      const model = buildSessionDetailModel(
        sessionResource({ state, totalClients: 0, stopAllowed: false }),
        sessionStatus({ state, runtimeState: state, totalClients: 0, stopAllowed: false }),
      );

      expect(model.actions).toMatchObject({
        canConnectPreview: true,
        connectPreviewLabel: 'Start and connect',
        canStop: false,
      });
      expect(model.actions.connectPreviewDescription).toContain('persisted browser profile');
    }
  });

  it('surfaces the session recording policy in runtime details', () => {
    const model = buildSessionDetailModel(
      sessionResource({ recordingMode: 'always', recordingRetentionSec: 3600 }),
      sessionStatus({
        recordingState: 'recording',
        activeRecordingId: 'recording-123',
        recorderAttached: true,
        recordingBytes: 4096,
        recordingDurationMs: 3000,
      }),
    );

    const runtimeSection = model.sections.find((section) => section.testId === 'session-detail-runtime-section');
    expect(runtimeSection?.facts).toContainEqual(expect.objectContaining({
      label: 'Recording policy',
      value: 'always / webm, retention 1h 0m',
      testId: 'session-detail-recording',
      tone: 'success',
    }));
    expect(runtimeSection?.facts).toContainEqual(expect.objectContaining({
      label: 'Recording state',
      value: 'recording, 4096 bytes, 3s',
      testId: 'session-detail-recording-state',
      tone: 'warning',
    }));
    expect(runtimeSection?.facts).toContainEqual(expect.objectContaining({
      label: 'Active recording',
      value: 'recording-123',
      testId: 'session-detail-active-recording',
    }));
    expect(runtimeSection?.facts).toContainEqual(expect.objectContaining({
      label: 'Recorder attached',
      value: 'yes',
      testId: 'session-detail-recorder-attached',
      tone: 'success',
    }));
    expect(model.actions).toMatchObject({
      canEnableRecording: false,
      canDisableRecording: true,
    });
  });
});
