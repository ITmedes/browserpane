import { describe, expect, it } from 'vitest';
import {
  formatSessionFileBytes,
  formatSessionFileSource,
  shortSessionFileDigest,
} from './session-file-format';

describe('session file formatting', () => {
  it('formats byte counts for compact admin cards', () => {
    expect(formatSessionFileBytes(0)).toBe('0 B');
    expect(formatSessionFileBytes(1536)).toBe('1.5 KB');
  });

  it('formats source and digest labels without hiding unknown values', () => {
    expect(formatSessionFileSource('browser_upload')).toBe('browser upload');
    expect(shortSessionFileDigest('1234567890abcdef9999')).toBe('1234567890abcdef...');
  });
});
