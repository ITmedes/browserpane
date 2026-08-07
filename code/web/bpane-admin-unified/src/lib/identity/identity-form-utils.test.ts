import { describe, expect, it } from 'vitest';

import { labelsToText, parseIdentityLabels, shortIdentityId, splitIdentityList } from './identity-form-utils';

describe('identity form utilities', () => {
  it('normalizes lists and labels without duplicate values', () => {
    expect(splitIdentityList('read, write\nread')).toEqual(['read', 'write']);
    expect(parseIdentityLabels('team=platform\nenvironment=test')).toEqual({
      ok: true,
      value: { team: 'platform', environment: 'test' },
    });
    expect(labelsToText({ team: 'platform', environment: 'test' })).toBe(
      'environment=test\nteam=platform',
    );
  });

  it('rejects malformed labels and shortens only long identifiers', () => {
    expect(parseIdentityLabels('team')).toEqual({ ok: false, error: 'Labels must use key=value format.' });
    expect(parseIdentityLabels('team=')).toEqual({
      ok: false,
      error: 'Labels must include a non-empty key and value.',
    });
    expect(shortIdentityId('short-id')).toBe('short-id');
    expect(shortIdentityId('12345678901234567890')).toBe('12345678...67890');
  });
});
