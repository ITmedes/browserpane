import { spawnSync } from 'node:child_process';

export class DockerCommandRunner {
  #execute;

  constructor(execute = spawnSync) {
    this.#execute = execute;
  }

  run(args, options = {}) {
    const result = this.#execute('docker', args, {
      cwd: options.cwd,
      encoding: 'utf8',
      env: options.env,
      input: options.input,
      maxBuffer: 16 * 1024 * 1024,
    });
    if (result.error || result.status !== 0) {
      const detail = result.error?.message ?? result.stderr?.trim() ?? 'unknown failure';
      throw new Error(`Docker ${args[0]} failed: ${detail}`);
    }
    const output = options.includeStderr
      ? `${result.stdout ?? ''}\n${result.stderr ?? ''}`
      : result.stdout ?? '';
    return output.trim();
  }

  tryRun(args, options = {}) {
    try {
      return this.run(args, options);
    } catch {
      return '';
    }
  }
}
