type FileSelectionHandler = (files: FileList | Iterable<File>) => void;

type FileTransferDomBindingsInput = {
  container: HTMLElement;
  enabled: boolean;
  onFilesSelected: FileSelectionHandler;
};

export class FileTransferDomBindings {
  private readonly container: HTMLElement;
  private enabled: boolean;
  private readonly onFilesSelected: FileSelectionHandler;
  private fileInput: HTMLInputElement | null = null;
  private dragDepth = 0;

  private readonly handleInputChange = (): void => {
    const files = this.fileInput?.files;
    if (files) {
      this.onFilesSelected(files);
    }
  };

  private readonly handleDragEnter = (event: DragEvent): void => {
    if (!this.enabled || !hasFilePayload(event)) return;
    event.preventDefault();
    this.dragDepth += 1;
  };

  private readonly handleDragOver = (event: DragEvent): void => {
    if (!this.enabled || !hasFilePayload(event)) return;
    event.preventDefault();
    if (event.dataTransfer) {
      event.dataTransfer.dropEffect = 'copy';
    }
  };

  private readonly handleDragLeave = (event: DragEvent): void => {
    if (!this.enabled || !hasFilePayload(event)) return;
    event.preventDefault();
    this.dragDepth = Math.max(0, this.dragDepth - 1);
  };

  private readonly handleDrop = (event: DragEvent): void => {
    if (!this.enabled || !hasFilePayload(event)) return;
    event.preventDefault();
    this.dragDepth = 0;
    const files = event.dataTransfer?.files;
    if (files) {
      this.onFilesSelected(files);
    }
  };

  constructor(input: FileTransferDomBindingsInput) {
    this.container = input.container;
    this.enabled = input.enabled;
    this.onFilesSelected = input.onFilesSelected;
    this.setup();
  }

  destroy(): void {
    this.container.removeEventListener('dragenter', this.handleDragEnter);
    this.container.removeEventListener('dragover', this.handleDragOver);
    this.container.removeEventListener('dragleave', this.handleDragLeave);
    this.container.removeEventListener('drop', this.handleDrop);
    if (this.fileInput) {
      this.fileInput.removeEventListener('change', this.handleInputChange);
      if (this.fileInput.parentNode) {
        this.fileInput.parentNode.removeChild(this.fileInput);
      }
      this.fileInput = null;
    }
  }

  setEnabled(enabled: boolean): void {
    this.enabled = enabled;
    if (this.fileInput) {
      this.fileInput.disabled = !enabled;
    }
  }

  promptUpload(): void {
    if (!this.enabled || !this.fileInput) return;
    this.fileInput.value = '';
    this.fileInput.click();
  }

  private setup(): void {
    this.fileInput = document.createElement('input');
    this.fileInput.type = 'file';
    this.fileInput.multiple = true;
    this.fileInput.style.display = 'none';
    this.fileInput.disabled = !this.enabled;
    this.fileInput.addEventListener('change', this.handleInputChange);
    this.container.appendChild(this.fileInput);

    this.container.addEventListener('dragenter', this.handleDragEnter);
    this.container.addEventListener('dragover', this.handleDragOver);
    this.container.addEventListener('dragleave', this.handleDragLeave);
    this.container.addEventListener('drop', this.handleDrop);
  }
}

function hasFilePayload(event: DragEvent): boolean {
  const types = event.dataTransfer?.types;
  return !!types && Array.from(types).includes('Files');
}
