import { createHash, randomUUID } from 'node:crypto';

const NAMESPACE_PATTERN = /^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/;
const MAX_NAMESPACE_LENGTH = 63;

export class ComposeNamespaceFactory {
  #randomId;

  constructor(randomId = randomUUID) {
    this.#randomId = randomId;
  }

  run(lane, environment = process.env) {
    const configured = environment.BPANE_CI_RUN_NAMESPACE;
    if (configured) return validateComposeNamespace(configured);

    const runId = environment.GITHUB_RUN_ID ?? this.#randomId().replaceAll('-', '').slice(0, 12);
    const attempt = environment.GITHUB_RUN_ATTEMPT ?? '1';
    return boundedNamespace(['bpane', runId, attempt, lane]);
  }

  stage(runNamespace, stageId) {
    validateComposeNamespace(runNamespace);
    return boundedNamespace([runNamespace, stageId]);
  }
}

export function validateComposeNamespace(namespace) {
  if (typeof namespace !== 'string' || !NAMESPACE_PATTERN.test(namespace)) {
    throw new TypeError('Compose namespace must contain 1-63 lowercase alphanumeric or hyphen characters.');
  }
  return namespace;
}

function boundedNamespace(parts) {
  const candidate = parts.map(sanitizePart).filter(Boolean).join('-');
  if (candidate.length <= MAX_NAMESPACE_LENGTH) return validateComposeNamespace(candidate);
  const digest = createHash('sha256').update(candidate).digest('hex').slice(0, 10);
  const prefix = candidate.slice(0, MAX_NAMESPACE_LENGTH - digest.length - 1).replace(/-+$/u, '');
  return validateComposeNamespace(`${prefix}-${digest}`);
}

function sanitizePart(value) {
  return String(value)
    .toLowerCase()
    .replace(/[^a-z0-9]+/gu, '-')
    .replace(/^-+|-+$/gu, '');
}
