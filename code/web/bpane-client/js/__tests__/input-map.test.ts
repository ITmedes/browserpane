import { describe, it, expect } from 'vitest';
import {
  domCodeToEvdev,
  buildModifiers,
  normalizeScroll,
  createScrollState,
  MOD_CTRL, MOD_ALT, MOD_SHIFT, MOD_META,
} from '../input-map.js';

describe('domCodeToEvdev', () => {
  it('maps letter keys correctly', () => {
    expect(domCodeToEvdev('KeyA')).toBe(30);
    expect(domCodeToEvdev('KeyZ')).toBe(44);
    expect(domCodeToEvdev('KeyQ')).toBe(16);
    expect(domCodeToEvdev('KeyM')).toBe(50);
  });

  it('maps digit keys correctly', () => {
    expect(domCodeToEvdev('Digit0')).toBe(11);
    expect(domCodeToEvdev('Digit1')).toBe(2);
    expect(domCodeToEvdev('Digit9')).toBe(10);
  });

  it('maps special keys', () => {
    expect(domCodeToEvdev('Escape')).toBe(1);
    expect(domCodeToEvdev('Backspace')).toBe(14);
    expect(domCodeToEvdev('Tab')).toBe(15);
    expect(domCodeToEvdev('Enter')).toBe(28);
    expect(domCodeToEvdev('Space')).toBe(57);
    expect(domCodeToEvdev('CapsLock')).toBe(58);
  });

  it('maps modifier keys', () => {
    expect(domCodeToEvdev('ShiftLeft')).toBe(42);
    expect(domCodeToEvdev('ShiftRight')).toBe(54);
    expect(domCodeToEvdev('ControlLeft')).toBe(29);
    expect(domCodeToEvdev('ControlRight')).toBe(97);
    expect(domCodeToEvdev('AltLeft')).toBe(56);
    expect(domCodeToEvdev('AltRight')).toBe(100);
    expect(domCodeToEvdev('MetaLeft')).toBe(125);
    expect(domCodeToEvdev('MetaRight')).toBe(126);
  });

  it('maps arrow keys', () => {
    expect(domCodeToEvdev('ArrowUp')).toBe(103);
    expect(domCodeToEvdev('ArrowDown')).toBe(108);
    expect(domCodeToEvdev('ArrowLeft')).toBe(105);
    expect(domCodeToEvdev('ArrowRight')).toBe(106);
  });

  it('maps function keys', () => {
    expect(domCodeToEvdev('F1')).toBe(59);
    expect(domCodeToEvdev('F10')).toBe(68);
    expect(domCodeToEvdev('F11')).toBe(87);
    expect(domCodeToEvdev('F12')).toBe(88);
  });

  it('maps navigation keys', () => {
    expect(domCodeToEvdev('Home')).toBe(102);
    expect(domCodeToEvdev('End')).toBe(107);
    expect(domCodeToEvdev('PageUp')).toBe(104);
    expect(domCodeToEvdev('PageDown')).toBe(109);
    expect(domCodeToEvdev('Delete')).toBe(111);
    expect(domCodeToEvdev('Insert')).toBe(110);
  });

  it('maps punctuation keys', () => {
    expect(domCodeToEvdev('Minus')).toBe(12);
    expect(domCodeToEvdev('Equal')).toBe(13);
    expect(domCodeToEvdev('BracketLeft')).toBe(26);
    expect(domCodeToEvdev('BracketRight')).toBe(27);
    expect(domCodeToEvdev('Backslash')).toBe(43);
    expect(domCodeToEvdev('Semicolon')).toBe(39);
    expect(domCodeToEvdev('Quote')).toBe(40);
    expect(domCodeToEvdev('Backquote')).toBe(41);
    expect(domCodeToEvdev('Comma')).toBe(51);
    expect(domCodeToEvdev('Period')).toBe(52);
    expect(domCodeToEvdev('Slash')).toBe(53);
  });

  it('returns undefined for unmapped keys', () => {
    expect(domCodeToEvdev('UnknownKey')).toBeUndefined();
    expect(domCodeToEvdev('')).toBeUndefined();
    expect(domCodeToEvdev('NumpadEnter')).toBe(96);
  });
});

describe('buildModifiers', () => {
  it('returns 0 when no modifiers', () => {
    expect(buildModifiers(false, false, false, false)).toBe(0);
  });

  it('sets individual modifier bits', () => {
    expect(buildModifiers(true, false, false, false)).toBe(MOD_CTRL);
    expect(buildModifiers(false, true, false, false)).toBe(MOD_ALT);
    expect(buildModifiers(false, false, true, false)).toBe(MOD_SHIFT);
    expect(buildModifiers(false, false, false, true)).toBe(MOD_META);
  });

  it('combines multiple modifiers', () => {
    expect(buildModifiers(true, true, false, false)).toBe(MOD_CTRL | MOD_ALT);
    expect(buildModifiers(true, true, true, true)).toBe(MOD_CTRL | MOD_ALT | MOD_SHIFT | MOD_META);
    expect(buildModifiers(false, false, true, true)).toBe(MOD_SHIFT | MOD_META);
  });

  it('uses correct bit values', () => {
    expect(MOD_CTRL).toBe(0x01);
    expect(MOD_ALT).toBe(0x02);
    expect(MOD_SHIFT).toBe(0x04);
    expect(MOD_META).toBe(0x08);
  });
});

describe('normalizeScroll', () => {
  it('returns zero for sub-threshold pixel deltas', () => {
    const state = createScrollState();
    // A small pixel delta shouldn't produce a full step
    const { dx, dy } = normalizeScroll(10, 10, 0, state);
    expect(dx).toBe(0);
    expect(Math.abs(dy)).toBe(0); // may be -0 due to inversion
  });

  it('accumulates pixel deltas across calls', () => {
    const state = createScrollState();
    let totalDy = 0;
    for (let i = 0; i < 6; i++) {
      const { dy } = normalizeScroll(0, 10, 0, state);
      totalDy += dy;
    }
    expect(totalDy).toBe(-1);
  });

  it('handles line-mode scroll (deltaMode=1)', () => {
    const state = createScrollState();
    const { dy } = normalizeScroll(0, 1, 1, state);
    expect(dy).toBe(-1);
  });

  it('handles page-mode scroll (deltaMode=2)', () => {
    const state = createScrollState();
    const { dx, dy } = normalizeScroll(0, 1, 2, state);
    expect(dy).toBe(-6);
  });

  it('inverts Y axis for protocol convention', () => {
    const state = createScrollState();
    // Large positive deltaY (scroll down) should produce negative dy
    const { dy } = normalizeScroll(0, 200, 0, state);
    expect(dy).toBeLessThanOrEqual(0);
  });

  it('handles horizontal scrolling', () => {
    const state = createScrollState();
    const { dx } = normalizeScroll(60, 0, 0, state);
    expect(dx).toBe(1);
  });

  it('preserves remainder across calls', () => {
    const state = createScrollState();
    // Send several small deltas — the state should accumulate
    normalizeScroll(0, 50, 0, state);
    expect(state.remainderY).not.toBe(0);
  });

  it('fresh state starts at zero', () => {
    const state = createScrollState();
    expect(state.remainderX).toBe(0);
    expect(state.remainderY).toBe(0);
  });
});
