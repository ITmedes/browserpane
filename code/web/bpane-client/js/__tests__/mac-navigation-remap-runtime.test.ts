import { describe, expect, it } from 'vitest';
import { MacNavigationRemapRuntime } from '../input/mac-navigation-remap-runtime.js';

describe('MacNavigationRemapRuntime', () => {
  it('starts remaps for left and right arrows and returns the correct target key', () => {
    const runtime = new MacNavigationRemapRuntime();

    expect(runtime.begin('ArrowLeft', ['ControlLeft'])).toEqual({
      remapCode: 'Home',
    });
    expect(runtime.begin('ArrowRight', ['ControlRight'])).toEqual({
      remapCode: 'End',
    });
  });

  it('ignores duplicate and unsupported remap keys', () => {
    const runtime = new MacNavigationRemapRuntime();

    expect(runtime.begin('ArrowLeft', [])).toEqual({
      remapCode: 'Home',
    });
    expect(runtime.begin('ArrowLeft', [])).toBeNull();
    expect(runtime.begin('KeyA', [])).toBeNull();
  });

  it('releases tracked remaps on keyup and then clears their state', () => {
    const runtime = new MacNavigationRemapRuntime();
    runtime.begin('ArrowLeft', ['ControlLeft']);

    expect(runtime.handleKeyup('ArrowLeft')).toEqual({
      releasedCtrlCodes: ['ControlLeft'],
    });
    expect(runtime.handleKeyup('ArrowLeft')).toBeNull();
  });

  it('clears all tracked remaps on reset', () => {
    const runtime = new MacNavigationRemapRuntime();
    runtime.begin('ArrowLeft', ['ControlLeft']);
    runtime.begin('ArrowRight', ['ControlRight']);

    runtime.reset();

    expect(runtime.handleKeyup('ArrowLeft')).toBeNull();
    expect(runtime.handleKeyup('ArrowRight')).toBeNull();
  });
});
