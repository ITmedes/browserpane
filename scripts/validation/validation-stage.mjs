export class ValidationStage {
  constructor({ id, description, command, args = [], cwd, timeoutSeconds }) {
    if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(id)) {
      throw new Error(`invalid validation stage id: ${id}`);
    }
    if (!Number.isInteger(timeoutSeconds) || timeoutSeconds <= 0) {
      throw new Error(`invalid timeout for validation stage: ${id}`);
    }
    this.id = id;
    this.description = description;
    this.command = command;
    this.args = Object.freeze([...args]);
    this.cwd = cwd;
    this.timeoutMs = timeoutSeconds * 1000;
    Object.freeze(this);
  }

  commandLine() {
    return [this.command, ...this.args].map((value) => this.#quote(value)).join(' ');
  }

  #quote(value) {
    const text = String(value);
    return /^[A-Za-z0-9_./:@=-]+$/.test(text)
      ? text
      : `'${text.replaceAll("'", "'\\''")}'`;
  }
}
