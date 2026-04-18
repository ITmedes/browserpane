import { describe, expect, it } from 'vitest';
import { SyntheticDeadAccentRuntime } from '../input/synthetic-dead-accent-runtime.js';

function createEvent(input: {
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

describe('SyntheticDeadAccentRuntime', () => {
  it('keeps the accent pending across dead-key release and emits the composed character on base keyup', () => {
    const runtime = new SyntheticDeadAccentRuntime();
    runtime.begin('acute', 'Equal');

    expect(runtime.handleKeyup('Equal')).toEqual({
      handled: true,
      clearDeadKeyCode: false,
    });

    expect(runtime.handleKeydown(createEvent({ code: 'KeyE', key: 'e' }))).toEqual({
      handled: true,
      clearKeyboardSink: true,
    });

    expect(runtime.handleKeyup('KeyE')).toEqual({
      handled: true,
      emitCharacter: { code: 'KeyE', key: 'é' },
      clearDeadKeyCode: true,
    });
  });

  it('returns a spacing-accent fallback and keeps dead-key suppression until the dead key is released', () => {
    const runtime = new SyntheticDeadAccentRuntime();
    runtime.begin('acute', 'Equal');

    expect(runtime.handleKeydown(createEvent({ code: 'KeyY', key: 'y' }))).toEqual({
      handled: false,
      fallback: {
        deadCode: 'Equal',
        spacingAccent: '´',
        deadKeyCode: 'Equal',
      },
    });

    expect(runtime.handleKeyup('Equal')).toEqual({
      handled: false,
      clearDeadKeyCode: false,
    });
  });

  it('clears dead-key suppression immediately when fallback happens after the dead key was already released', () => {
    const runtime = new SyntheticDeadAccentRuntime();
    runtime.begin('acute', 'Equal');

    expect(runtime.handleKeyup('Equal')).toEqual({
      handled: true,
      clearDeadKeyCode: false,
    });

    expect(runtime.handleKeydown(createEvent({ code: 'KeyY', key: 'y' }))).toEqual({
      handled: false,
      fallback: {
        deadCode: 'Equal',
        spacingAccent: '´',
        deadKeyCode: null,
      },
    });
  });

  it('ignores repeated base-key keydowns and clears only after the dead key is released', () => {
    const runtime = new SyntheticDeadAccentRuntime();
    runtime.begin('acute', 'Equal');

    expect(runtime.handleKeydown(createEvent({ code: 'KeyE', key: 'e' }))).toEqual({
      handled: true,
      clearKeyboardSink: true,
    });

    expect(runtime.handleKeydown(createEvent({ code: 'KeyE', key: 'e' }))).toEqual({
      handled: true,
      clearKeyboardSink: false,
    });

    expect(runtime.handleKeyup('KeyE')).toEqual({
      handled: true,
      emitCharacter: { code: 'KeyE', key: 'é' },
      clearDeadKeyCode: false,
    });

    expect(runtime.handleKeyup('Equal')).toEqual({
      handled: true,
      clearDeadKeyCode: true,
    });
  });
});
