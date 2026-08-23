import fs from 'node:fs';

export class ComposeHarnessRecorder {
  #filePath;

  constructor(filePath) {
    this.#filePath = filePath;
  }

  record(event) {
    if (!this.#filePath) return;
    const bounded = sanitizeEvent(event);
    fs.appendFileSync(this.#filePath, `${JSON.stringify(bounded)}\n`, {
      encoding: 'utf8',
      mode: 0o600,
    });
  }

  load() {
    if (!this.#filePath || !fs.existsSync(this.#filePath)) return [];
    return fs.readFileSync(this.#filePath, 'utf8').split('\n').filter(Boolean).slice(0, 256)
      .map((line) => sanitizeEvent(JSON.parse(line)));
  }
}

function sanitizeEvent(event) {
  const serialized = JSON.stringify(event ?? {});
  if (Buffer.byteLength(serialized) > 16_384) {
    return { type: 'harness', outcome: 'event_too_large' };
  }
  if (/(?:bearer|password|private[_ -]?key|secret|token|https?:\/\/)/iu.test(serialized)) {
    return { type: 'harness', outcome: 'event_redacted' };
  }
  return JSON.parse(serialized);
}
