export class WorkerRequestTimeoutError extends Error {
  readonly code = "worker_request_timeout";
  readonly operation: string;
  readonly timeoutMs: number;

  constructor(operation: string, timeoutMs: number) {
    super(`${operation} timed out after ${timeoutMs} ms`);
    this.name = "WorkerRequestTimeoutError";
    this.operation = operation;
    this.timeoutMs = timeoutMs;
  }
}

export class HttpRequestDeadline {
  private readonly timeoutMs: number;

  constructor(timeoutMs: number) {
    if (!Number.isSafeInteger(timeoutMs) || timeoutMs <= 0) {
      throw new Error("worker request timeout must be a positive integer");
    }
    this.timeoutMs = timeoutMs;
  }

  async run<T>(
    operation: string,
    request: (signal: AbortSignal) => Promise<T>,
    parentSignal?: AbortSignal | null,
  ): Promise<T> {
    const controller = new AbortController();
    const timeoutError = new WorkerRequestTimeoutError(operation, this.timeoutMs);
    const abortFromParent = (): void => controller.abort(parentSignal?.reason);

    if (parentSignal?.aborted) {
      abortFromParent();
    } else {
      parentSignal?.addEventListener("abort", abortFromParent, { once: true });
    }

    let timeout: NodeJS.Timeout | undefined;
    const deadline = new Promise<never>((_resolve, reject) => {
      timeout = setTimeout(() => {
        controller.abort(timeoutError);
        reject(timeoutError);
      }, this.timeoutMs);
    });

    try {
      return await Promise.race([request(controller.signal), deadline]);
    } finally {
      if (timeout) {
        clearTimeout(timeout);
      }
      parentSignal?.removeEventListener("abort", abortFromParent);
    }
  }
}
