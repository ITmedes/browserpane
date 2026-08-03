const MAX_SECTION_CHARACTERS = 500_000;

export class ComposeDiagnosticsCollector {
  #executor;
  #redactor;

  constructor(executor, redactor) {
    this.#executor = executor;
    this.#redactor = redactor;
  }

  collect(rootDirectory) {
    const composeFile = `${rootDirectory}/deploy/compose.yml`;
    const commands = [
      {
        title: 'Compose service status',
        args: ['compose', '-f', composeFile, 'ps', '--all', '--format',
          'table {{.Name}}\t{{.Service}}\t{{.State}}\t{{.Status}}']
      },
      {
        title: 'Control-plane service logs (last 300 lines)',
        args: ['compose', '-f', composeFile, 'logs', '--no-color', '--timestamps',
          '--tail', '300', 'gateway', 'host', 'web', 'mcp-bridge', 'keycloak',
          'postgres', 'vault']
      },
      {
        title: 'Short-lived BrowserPane container status',
        args: ['ps', '--all', '--filter', 'name=bpane-runtime-', '--filter',
          'name=bpane-workflow-', '--format',
          'table {{.Names}}\t{{.Image}}\t{{.State}}\t{{.Status}}']
      }
    ];
    return commands.map((command) => this.#capture(command, rootDirectory)).join('\n\n');
  }

  #capture(command, cwd) {
    const result = this.#executor.run('docker', command.args, cwd);
    const raw = [result.stdout, result.stderr, result.error]
      .filter(Boolean)
      .join('\n');
    const sanitized = this.#redactor.redact(raw);
    const bounded = sanitized.length > MAX_SECTION_CHARACTERS
      ? `${sanitized.slice(0, MAX_SECTION_CHARACTERS)}\n<truncated>`
      : sanitized;
    return [
      `## ${command.title}`,
      `exit_status=${result.status ?? 'unavailable'}`,
      bounded || '<no output>'
    ].join('\n');
  }
}
