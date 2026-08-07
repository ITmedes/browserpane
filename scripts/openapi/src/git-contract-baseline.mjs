import { spawnSync } from 'node:child_process';

export class GitContractBaseline {
  constructor(rootDirectory, execute = spawnSync) {
    this.rootDirectory = rootDirectory;
    this.execute = execute;
  }

  load(baseRef, relativeContractPath) {
    this.#verify(baseRef);
    const result = this.#run(['show', `${baseRef}:${relativeContractPath}`]);
    if (result.status !== 0) {
      throw new Error(`unable to load OpenAPI baseline at ${baseRef}: ${this.#error(result)}`);
    }
    return result.stdout;
  }

  #verify(baseRef) {
    const result = this.#run(['rev-parse', '--verify', `${baseRef}^{commit}`]);
    if (result.status !== 0) {
      throw new Error(`OpenAPI base ref is not a commit: ${baseRef}`);
    }
  }

  #run(args) {
    const result = this.execute('git', args, {
      cwd: this.rootDirectory,
      encoding: 'utf8',
      maxBuffer: 30 * 1024 * 1024
    });
    if (result.error) throw new Error(`unable to execute git: ${result.error.message}`);
    return result;
  }

  #error(result) {
    return String(result.stderr || result.stdout || `exit ${result.status}`).trim().slice(0, 500);
  }
}
