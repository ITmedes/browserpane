import { describe, expect, it, vi } from 'vitest';
import { SuppressedKeyupTracker } from '../input/suppressed-keyup-tracker.js';

describe('SuppressedKeyupTracker', () => {
  it('returns false when clearing a code that is not suppressed', () => {
    const tracker = new SuppressedKeyupTracker({
      timeoutMs: 750,
      setTimeoutFn: window.setTimeout,
      clearTimeoutFn: window.clearTimeout,
    });

    expect(tracker.clear('KeyV')).toBe(false);
  });

  it('clears a suppressed keyup and cancels its timeout', () => {
    const clearTimeoutFn = vi.fn(window.clearTimeout);
    const tracker = new SuppressedKeyupTracker({
      timeoutMs: 750,
      setTimeoutFn: window.setTimeout,
      clearTimeoutFn,
    });

    tracker.suppress('KeyV');

    expect(tracker.clear('KeyV')).toBe(true);
    expect(clearTimeoutFn).toHaveBeenCalledTimes(1);
    expect(tracker.clear('KeyV')).toBe(false);
  });

  it('reschedules duplicate suppression and expires automatically', async () => {
    vi.useFakeTimers();
    const tracker = new SuppressedKeyupTracker({
      timeoutMs: 750,
      setTimeoutFn: window.setTimeout,
      clearTimeoutFn: window.clearTimeout,
    });

    tracker.suppress('KeyV');
    await vi.advanceTimersByTimeAsync(500);
    tracker.suppress('KeyV');

    expect(tracker.clear('KeyV')).toBe(true);

    tracker.suppress('KeyV');
    await vi.advanceTimersByTimeAsync(800);

    expect(tracker.clear('KeyV')).toBe(false);
  });

  it('reset clears all pending suppressions', () => {
    const clearTimeoutFn = vi.fn(window.clearTimeout);
    const tracker = new SuppressedKeyupTracker({
      timeoutMs: 750,
      setTimeoutFn: window.setTimeout,
      clearTimeoutFn,
    });

    tracker.suppress('KeyV');
    tracker.suppress('KeyC');
    tracker.reset();

    expect(clearTimeoutFn).toHaveBeenCalledTimes(2);
    expect(tracker.clear('KeyV')).toBe(false);
    expect(tracker.clear('KeyC')).toBe(false);
  });
});
