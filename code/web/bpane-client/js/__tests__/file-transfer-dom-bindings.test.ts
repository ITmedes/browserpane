import { afterEach, describe, expect, it, vi } from 'vitest';

import { FileTransferDomBindings } from '../file-transfer/dom-bindings.js';

type DragEventTransfer = {
  files?: FileList;
  types?: readonly string[];
  dropEffect?: string;
};

function createDragEvent(type: string, dataTransfer?: DragEventTransfer): DragEvent {
  const event = new Event(type, { bubbles: true, cancelable: true }) as DragEvent;
  Object.defineProperty(event, 'dataTransfer', {
    configurable: true,
    value: dataTransfer as unknown as DataTransfer,
  });
  return event;
}

describe('FileTransferDomBindings', () => {
  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  it('creates a hidden file input and prompts it when enabled', () => {
    const container = document.createElement('div');
    document.body.appendChild(container);
    const onFilesSelected = vi.fn();
    const click = vi.spyOn(HTMLInputElement.prototype, 'click').mockImplementation(() => {});
    const bindings = new FileTransferDomBindings({
      container,
      enabled: true,
      onFilesSelected,
    });

    const input = container.querySelector('input[type="file"]') as HTMLInputElement | null;
    expect(input).not.toBeNull();
    expect(input?.multiple).toBe(true);
    expect(input?.style.display).toBe('none');

    if (input) {
      Object.defineProperty(input, 'value', {
        configurable: true,
        writable: true,
        value: 'stale-value',
      });
    }
    bindings.promptUpload();

    expect(input?.value).toBe('');
    expect(click).toHaveBeenCalledTimes(1);

    bindings.destroy();
  });

  it('keeps the input disabled and suppresses prompt and drop handling when disabled', () => {
    const container = document.createElement('div');
    document.body.appendChild(container);
    const onFilesSelected = vi.fn();
    const click = vi.spyOn(HTMLInputElement.prototype, 'click').mockImplementation(() => {});
    const bindings = new FileTransferDomBindings({
      container,
      enabled: false,
      onFilesSelected,
    });

    const input = container.querySelector('input[type="file"]') as HTMLInputElement | null;
    expect(input?.disabled).toBe(true);

    bindings.promptUpload();
    expect(click).not.toHaveBeenCalled();

    const dropEvent = createDragEvent('drop', {
      files: [new File(['a'], 'a.txt')] as unknown as FileList,
      types: ['Files'],
      dropEffect: 'none',
    });
    container.dispatchEvent(dropEvent);

    expect(dropEvent.defaultPrevented).toBe(false);
    expect(onFilesSelected).not.toHaveBeenCalled();

    bindings.destroy();
  });

  it('forwards dropped files and marks drag events as copy operations when enabled', () => {
    const container = document.createElement('div');
    document.body.appendChild(container);
    const onFilesSelected = vi.fn();
    const bindings = new FileTransferDomBindings({
      container,
      enabled: true,
      onFilesSelected,
    });

    const files = [new File(['hello'], 'report.txt')] as unknown as FileList;
    const dragOverEvent = createDragEvent('dragover', {
      files,
      types: ['Files'],
      dropEffect: 'none',
    });
    container.dispatchEvent(dragOverEvent);

    expect(dragOverEvent.defaultPrevented).toBe(true);
    expect(dragOverEvent.dataTransfer?.dropEffect).toBe('copy');

    const dropEvent = createDragEvent('drop', {
      files,
      types: ['Files'],
      dropEffect: 'none',
    });
    container.dispatchEvent(dropEvent);

    expect(dropEvent.defaultPrevented).toBe(true);
    expect(onFilesSelected).toHaveBeenCalledWith(files);

    bindings.destroy();
  });

  it('removes the file input on destroy', () => {
    const container = document.createElement('div');
    document.body.appendChild(container);
    const bindings = new FileTransferDomBindings({
      container,
      enabled: true,
      onFilesSelected: vi.fn(),
    });

    expect(container.querySelector('input[type="file"]')).not.toBeNull();

    bindings.destroy();

    expect(container.querySelector('input[type="file"]')).toBeNull();
  });
});
