export interface CompositionTextRuntimeInput {
  commitText: (text: string) => void;
  getKeyboardSinkValue: () => string;
  clearKeyboardSink: () => void;
  documentLike?: Document;
}

export interface CompositionTextBindingInput {
  keyboardTarget: HTMLTextAreaElement;
  signal: AbortSignal;
}

export class CompositionTextRuntime {
  private readonly commitText: (text: string) => void;
  private readonly getKeyboardSinkValue: () => string;
  private readonly clearKeyboardSink: () => void;
  private readonly documentLike?: Document;

  constructor(input: CompositionTextRuntimeInput) {
    this.commitText = input.commitText;
    this.getKeyboardSinkValue = input.getKeyboardSinkValue;
    this.clearKeyboardSink = input.clearKeyboardSink;
    this.documentLike = input.documentLike;
  }

  bind(input: CompositionTextBindingInput): void {
    input.keyboardTarget.addEventListener('compositionend', this.handleCompositionEnd, { signal: input.signal });
    input.keyboardTarget.addEventListener('input', this.handleInput, { signal: input.signal });
    this.documentLike?.addEventListener('compositionend', this.handleCompositionEnd, {
      capture: true,
      signal: input.signal,
    });
  }

  private readonly handleCompositionEnd = (event: Event): void => {
    const compositionEvent = event as CompositionEvent;
    this.commitText(compositionEvent.data ?? '');
    this.clearKeyboardSink();
  };

  private readonly handleInput = (event: Event): void => {
    const inputEvent = event as InputEvent;
    const text = inputEvent.data ?? this.getKeyboardSinkValue();
    this.commitText(text);
    this.clearKeyboardSink();
  };
}
