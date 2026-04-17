import { beforeEach, describe, expect, it } from 'vitest';
import { KeyboardSinkRuntime } from '../input/keyboard-sink-runtime.js';

beforeEach(() => {
  document.body.innerHTML = '';
});

describe('KeyboardSinkRuntime', () => {
  it('creates and reuses a hidden keyboard sink under the canvas parent', () => {
    const parent = document.createElement('div');
    const canvas = document.createElement('canvas');
    parent.appendChild(canvas);
    document.body.appendChild(parent);

    const runtime = new KeyboardSinkRuntime({
      canvas,
      documentLike: document,
    });

    const first = runtime.ensure();
    const second = runtime.ensure();

    expect(first).toBe(second);
    expect(first.parentElement).toBe(parent);
    expect(first.dataset.bpaneKeyboardSink).toBe('true');
    expect(first.getAttribute('aria-hidden')).toBe('true');
    expect(first.autocomplete).toBe('off');
    expect(first.autocapitalize).toBe('off');
    expect(first.spellcheck).toBe(false);
    expect(first.tabIndex).toBe(-1);
    expect(first.style.position).toBe('absolute');
    expect(first.style.left).toBe('-9999px');
    expect(first.style.width).toBe('1px');
    expect(first.style.height).toBe('1px');
    expect(first.style.opacity).toBe('0');
    expect(first.style.pointerEvents).toBe('none');
    expect(first.style.whiteSpace).toBe('pre');
  });

  it('falls back to document.body when the canvas has no parent element', () => {
    const canvas = document.createElement('canvas');
    const runtime = new KeyboardSinkRuntime({
      canvas,
      documentLike: document,
    });

    const sink = runtime.ensure();

    expect(sink.parentElement).toBe(document.body);
  });

  it('reads, clears, and removes the keyboard sink', () => {
    const canvas = document.createElement('canvas');
    document.body.appendChild(canvas);
    const runtime = new KeyboardSinkRuntime({
      canvas,
      documentLike: document,
    });

    const sink = runtime.ensure();
    sink.value = 'pending text';

    expect(runtime.getValue()).toBe('pending text');

    runtime.clear();
    expect(sink.value).toBe('');
    expect(runtime.getValue()).toBe('');

    runtime.destroy();
    expect(document.querySelector('textarea[data-bpane-keyboard-sink="true"]')).toBeNull();

    const recreated = runtime.ensure();
    expect(recreated).not.toBe(sink);
  });
});
