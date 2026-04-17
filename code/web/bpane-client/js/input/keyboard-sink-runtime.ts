export interface KeyboardSinkRuntimeInput {
  canvas: HTMLCanvasElement;
  documentLike?: Document;
}

export class KeyboardSinkRuntime {
  private readonly canvas: HTMLCanvasElement;
  private readonly documentLike?: Document;
  private sink: HTMLTextAreaElement | null = null;

  constructor(input: KeyboardSinkRuntimeInput) {
    this.canvas = input.canvas;
    this.documentLike = input.documentLike;
  }

  ensure(): HTMLTextAreaElement {
    if (this.sink) {
      return this.sink;
    }

    const documentLike = this.documentLike ?? document;
    const sink = documentLike.createElement('textarea');
    sink.setAttribute('data-bpane-keyboard-sink', 'true');
    sink.setAttribute('aria-hidden', 'true');
    sink.autocomplete = 'off';
    sink.autocapitalize = 'off';
    sink.spellcheck = false;
    sink.tabIndex = -1;
    sink.style.position = 'absolute';
    sink.style.left = '-9999px';
    sink.style.top = '0';
    sink.style.width = '1px';
    sink.style.height = '1px';
    sink.style.opacity = '0';
    sink.style.pointerEvents = 'none';
    sink.style.whiteSpace = 'pre';

    (this.canvas.parentElement ?? documentLike.body).appendChild(sink);
    this.sink = sink;
    return sink;
  }

  getValue(): string {
    return this.sink?.value ?? '';
  }

  clear(): void {
    if (this.sink) {
      this.sink.value = '';
    }
  }

  destroy(): void {
    if (this.sink) {
      this.sink.remove();
      this.sink = null;
    }
  }
}
