import { describe, expect, it } from 'vitest';
import {
  composeSyntheticDeadAccent,
  getSyntheticDeadAccentSpacingCharacter,
  resolveSupportedDeadAccent,
} from '../input/synthetic-dead-accent.js';

function createKeyEventLike(input: {
  code: string;
  key: string;
  altKey?: boolean;
  ctrlKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
}) {
  return {
    code: input.code,
    key: input.key,
    altKey: input.altKey ?? false,
    ctrlKey: input.ctrlKey ?? false,
    metaKey: input.metaKey ?? false,
    shiftKey: input.shiftKey ?? false,
  };
}

describe('synthetic dead accent helpers', () => {
  it('resolves supported dead accents for mac option and dedicated dead keys', () => {
    expect(resolveSupportedDeadAccent(createKeyEventLike({
      code: 'KeyE',
      key: 'Dead',
      altKey: true,
    }), true)).toBe('acute');
    expect(resolveSupportedDeadAccent(createKeyEventLike({
      code: 'KeyI',
      key: 'Dead',
      altKey: true,
    }), true)).toBe('circumflex');
    expect(resolveSupportedDeadAccent(createKeyEventLike({
      code: 'Backquote',
      key: 'Dead',
      altKey: true,
    }), true)).toBe('grave');
    expect(resolveSupportedDeadAccent(createKeyEventLike({
      code: 'Equal',
      key: 'Dead',
    }), true)).toBe('acute');
    expect(resolveSupportedDeadAccent(createKeyEventLike({
      code: 'Equal',
      key: 'Dead',
      shiftKey: true,
    }), true)).toBe('grave');
    expect(resolveSupportedDeadAccent(createKeyEventLike({
      code: 'IntlBackslash',
      key: 'Dead',
    }), true)).toBe('circumflex');
  });

  it('returns null for unsupported or disallowed dead-key combinations', () => {
    expect(resolveSupportedDeadAccent(createKeyEventLike({
      code: 'KeyE',
      key: 'Dead',
      altKey: true,
    }), false)).toBeNull();
    expect(resolveSupportedDeadAccent(createKeyEventLike({
      code: 'KeyE',
      key: 'Dead',
      altKey: true,
      ctrlKey: true,
    }), true)).toBeNull();
    expect(resolveSupportedDeadAccent(createKeyEventLike({
      code: 'KeyE',
      key: 'e',
      altKey: true,
    }), true)).toBeNull();
    expect(resolveSupportedDeadAccent(createKeyEventLike({
      code: 'Slash',
      key: 'Dead',
    }), true)).toBeNull();
  });

  it('composes vowels and spacing accents from supported synthetic accents', () => {
    expect(composeSyntheticDeadAccent('acute', createKeyEventLike({
      code: 'KeyE',
      key: 'e',
    }))).toBe('é');
    expect(composeSyntheticDeadAccent('grave', createKeyEventLike({
      code: 'KeyA',
      key: 'A',
      shiftKey: true,
    }))).toBe('À');
    expect(composeSyntheticDeadAccent('circumflex', createKeyEventLike({
      code: 'Space',
      key: ' ',
    }))).toBe('^');
  });

  it('falls back to code-based vowel normalization when key text is not printable', () => {
    expect(composeSyntheticDeadAccent('acute', createKeyEventLike({
      code: 'KeyO',
      key: 'Process',
    }))).toBe('ó');
    expect(composeSyntheticDeadAccent('acute', createKeyEventLike({
      code: 'KeyO',
      key: 'Process',
      shiftKey: true,
    }))).toBe('Ó');
  });

  it('returns null for unsupported bases and exposes spacing characters', () => {
    expect(composeSyntheticDeadAccent('acute', createKeyEventLike({
      code: 'KeyY',
      key: 'y',
    }))).toBeNull();
    expect(getSyntheticDeadAccentSpacingCharacter('acute')).toBe('´');
    expect(getSyntheticDeadAccentSpacingCharacter('grave')).toBe('`');
    expect(getSyntheticDeadAccentSpacingCharacter('circumflex')).toBe('^');
  });
});
