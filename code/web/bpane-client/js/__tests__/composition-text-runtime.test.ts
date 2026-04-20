import { beforeEach, describe, expect, it, vi } from 'vitest';
import { CompositionTextRuntime } from '../input/composition-text-runtime.js';

function dispatchCompositionEnd(target: EventTarget, data: string): void {
  const event = new CompositionEvent('compositionend', {
    bubbles: true,
    cancelable: false,
    data,
  });
  target.dispatchEvent(event);
}

function dispatchInput(target: HTMLTextAreaElement, data: string | null, value?: string): void {
  if (value !== undefined) {
    target.value = value;
  }
  target.dispatchEvent(new InputEvent('input', {
    bubbles: true,
    cancelable: false,
    data,
  }));
}

beforeEach(() => {
  document.body.innerHTML = '';
});

describe('CompositionTextRuntime', () => {
  it('commits compositionend text from the keyboard target and clears the sink', () => {
    const commitText = vi.fn();
    const clearKeyboardSink = vi.fn();
    const keyboardTarget = document.createElement('textarea');
    const runtime = new CompositionTextRuntime({
      commitText,
      getKeyboardSinkValue: () => keyboardTarget.value,
      clearKeyboardSink,
      documentLike: document,
    });
    const abortController = new AbortController();

    runtime.bind({
      keyboardTarget,
      signal: abortController.signal,
    });

    dispatchCompositionEnd(keyboardTarget, 'ô');

    expect(commitText).toHaveBeenCalledWith('ô');
    expect(clearKeyboardSink).toHaveBeenCalledTimes(1);
  });

  it('commits input event data and falls back to the keyboard sink value when needed', () => {
    const commitText = vi.fn();
    const clearKeyboardSink = vi.fn();
    const keyboardTarget = document.createElement('textarea');
    const runtime = new CompositionTextRuntime({
      commitText,
      getKeyboardSinkValue: () => keyboardTarget.value,
      clearKeyboardSink,
      documentLike: document,
    });
    const abortController = new AbortController();

    runtime.bind({
      keyboardTarget,
      signal: abortController.signal,
    });

    dispatchInput(keyboardTarget, 'é');
    dispatchInput(keyboardTarget, null, 'fallback text');

    expect(commitText).toHaveBeenNthCalledWith(1, 'é');
    expect(commitText).toHaveBeenNthCalledWith(2, 'fallback text');
    expect(clearKeyboardSink).toHaveBeenCalledTimes(2);
  });

  it('listens for document-level compositionend events in capture phase', () => {
    const commitText = vi.fn();
    const clearKeyboardSink = vi.fn();
    const keyboardTarget = document.createElement('textarea');
    document.body.appendChild(keyboardTarget);
    const runtime = new CompositionTextRuntime({
      commitText,
      getKeyboardSinkValue: () => keyboardTarget.value,
      clearKeyboardSink,
      documentLike: document,
    });
    const abortController = new AbortController();

    runtime.bind({
      keyboardTarget,
      signal: abortController.signal,
    });

    dispatchCompositionEnd(document, 'é');

    expect(commitText).toHaveBeenCalledWith('é');
    expect(clearKeyboardSink).toHaveBeenCalledTimes(1);
  });
});
