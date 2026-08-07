import { describe, expect, it } from 'vitest';

import {
  resolveSessionSubareaRoute,
  sessionSubareaHref,
  sessionSubareas,
} from './session-subarea';

describe('session subarea routes', () => {
  it('builds encoded canonical admin-new routes', () => {
    expect(sessionSubareaHref('session/with space', 'overview'))
      .toBe('/admin-new/sessions/session%2Fwith%20space');
    expect(sessionSubareaHref('session/with space', 'live'))
      .toBe('/admin-new/sessions/session%2Fwith%20space/live');
    expect(sessionSubareaHref('session-1', 'automation'))
      .toBe('/admin-new/sessions/session-1/automation');
    expect(sessionSubareaHref('session-1', 'policy'))
      .toBe('/admin-new/sessions/session-1/policy');
    expect(sessionSubareaHref('session-1', 'network'))
      .toBe('/admin-new/sessions/session-1/network');
  });

  it('resolves overview, subarea, preview, base-prefixed, and trailing-slash paths', () => {
    expect(resolveSessionSubareaRoute('/admin-new/sessions/session-1')).toEqual({
      sessionId: 'session-1',
      activeId: 'overview',
    });
    expect(resolveSessionSubareaRoute('/sessions/session%20one/live/')).toEqual({
      sessionId: 'session one',
      activeId: 'live',
    });
    expect(resolveSessionSubareaRoute('/admin-new/sessions/session-1/preview')).toEqual({
      sessionId: 'session-1',
      activeId: 'live',
    });
    expect(resolveSessionSubareaRoute('/admin-new/sessions/session-1/files')?.activeId).toBe('files');
    expect(resolveSessionSubareaRoute('/admin-new/sessions/session-1/automation')?.activeId)
      .toBe('automation');
    expect(resolveSessionSubareaRoute('/admin-new/sessions/session-1/policy')?.activeId)
      .toBe('policy');
  });

  it('rejects unrelated, nested, and malformed paths', () => {
    expect(resolveSessionSubareaRoute('/admin-new/sessions')).toBeNull();
    expect(resolveSessionSubareaRoute('/admin-new/sessions/session-1/live/more')).toBeNull();
    expect(resolveSessionSubareaRoute('/admin-new/sessions/%E0%A4%A/live')).toBeNull();
  });

  it('keeps the complete route contract in operator order', () => {
    expect(sessionSubareas.map((subarea) => subarea.id)).toEqual([
      'overview',
      'live',
      'automation',
      'policy',
      'files',
      'recordings',
      'network',
    ]);
  });
});
