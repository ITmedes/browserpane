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
});
