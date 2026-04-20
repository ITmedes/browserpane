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

  it('invokes bare timer methods with the global receiver', () => {
    const scheduled = new Map<number, () => void>();
    let nextTimerId = 1;
    let seenSetThis = false;
    let seenClearThis = false;
    const setTimeoutFn = function(this: unknown, handler: TimerHandler): number {
      seenSetThis = this === globalThis;
      if (this !== globalThis) {
        throw new TypeError('Illegal invocation');
      }
      if (typeof handler !== 'function') {
        throw new TypeError('Unexpected timer handler');
      }
      const callback = handler as () => void;
      const timerId = nextTimerId++;
      scheduled.set(timerId, callback);
      return timerId;
    };
    const clearTimeoutFn = function(this: unknown, timerId: number | undefined): void {
      seenClearThis = this === globalThis;
      if (this !== globalThis) {
        throw new TypeError('Illegal invocation');
      }
      if (timerId === undefined) {
        throw new TypeError('Unexpected timer id');
      }
      scheduled.delete(timerId);
    };

    const tracker = new SuppressedKeyupTracker({
      timeoutMs: 750,
      setTimeoutFn: setTimeoutFn as typeof window.setTimeout,
      clearTimeoutFn: clearTimeoutFn as typeof window.clearTimeout,
    });

    tracker.suppress('KeyV');
    expect(seenSetThis).toBe(true);
    expect(scheduled.size).toBe(1);
    expect(tracker.clear('KeyV')).toBe(true);
    expect(seenClearThis).toBe(true);
    expect(scheduled.size).toBe(0);
  });
});
