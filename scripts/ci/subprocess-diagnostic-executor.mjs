import { spawnSync } from 'node:child_process';

export class SubprocessDiagnosticExecutor {
  run(command, args, cwd) {
    const result = spawnSync(command, args, {
      cwd,
      encoding: 'utf8',
      maxBuffer: 2 * 1024 * 1024
    });
    return {
      status: result.status,
      stdout: result.stdout ?? '',
      stderr: result.stderr ?? '',
      error: result.error?.message ?? null
    };
  }
}
