const FAILURE_CLASSES = new Set(['product', 'harness', 'infrastructure', 'unknown']);

export class ComposeFailureClassifier {
  classify({ exitCode, signal, spawnError, requestedSignal, defaultClass }) {
    if (exitCode === 0 && !signal && !spawnError && !requestedSignal) {
      return { outcome: 'success', failureClass: null };
    }
    if (requestedSignal || signal) {
      return { outcome: 'cancelled', failureClass: 'infrastructure' };
    }
    if (spawnError) return { outcome: 'failure', failureClass: 'harness' };
    return {
      outcome: 'failure',
      failureClass: FAILURE_CLASSES.has(defaultClass) ? defaultClass : 'unknown',
    };
  }
}
