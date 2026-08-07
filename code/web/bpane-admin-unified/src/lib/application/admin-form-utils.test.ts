import { describe, expect, it } from 'vitest';

import { labelsToFormText, parseKeyValueLabels, splitFormEntries } from './admin-form-utils';

describe('admin form utilities', () => {
  it('parses newline and comma separated labels', () => {
    expect(parseKeyValueLabels('team=platform\nenvironment=test,region=eu')).toEqual({
      ok: true,
      value: { team: 'platform', environment: 'test', region: 'eu' },
    });
  });

  it('rejects malformed and empty labels', () => {
    expect(parseKeyValueLabels('team')).toEqual({
      ok: false,
      error: 'Labels must use key=value format.',
    });
    expect(parseKeyValueLabels('team=')).toEqual({
      ok: false,
      error: 'Labels must include a non-empty key and value.',
    });
  });

  it('normalizes entries and formats labels deterministically', () => {
    expect(splitFormEntries('one, two\none')).toEqual(['one', 'two']);
    expect(labelsToFormText({ z: 'last', a: 'first' })).toBe('a=first\nz=last');
  });
});
