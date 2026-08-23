import { validateComposeNamespace } from './compose-namespace.mjs';

const PROJECT_NAMESPACE_LENGTH = 32;

export function egressProjectNames(runNamespace = null) {
  const suffix = runNamespace
    ? `-${validateComposeNamespace(runNamespace).slice(0, PROJECT_NAMESPACE_LENGTH)}`
    : '';
  return Object.freeze({
    observer: `bpane-ci-egress${suffix}`,
    tls: `bpane-ci-egress-tls${suffix}`,
  });
}
