import { ControlApiError } from '../api/authenticated-api';

const WORKFLOW_SOURCE_MESSAGES: Readonly<Record<string, string>> = {
  workflow_source_invalid:
    'Workflow source is invalid. Check the repository URL, root path, entrypoint, and version settings.',
  workflow_source_ref_resolution_failed:
    'Workflow source ref could not be resolved. Check that the branch, tag, or commit exists.',
  workflow_source_repository_access_failed:
    'Workflow source repository is not reachable from the gateway. Check repository access, credentials, network reachability, and local git safe.directory configuration.',
  workflow_source_materialization_failed:
    'Workflow source checkout failed. Check that the pinned commit, root path, and entrypoint can be checked out.',
  workflow_source_snapshot_failed:
    'Workflow source snapshot could not be created. Check source file permissions and the local workflow snapshot workspace.',
  workflow_source_infrastructure_unavailable:
    'Workflow source tooling is unavailable in the gateway. Check the gateway image and local runtime dependencies.',
};

export function workflowOperationErrorMessage(value: unknown): string {
  if (value instanceof ControlApiError) {
    const workflowMessage = value.apiCode ? WORKFLOW_SOURCE_MESSAGES[value.apiCode] : undefined;
    if (workflowMessage) {
      return appendDetail(workflowMessage, value.recoveryHint ?? value.apiMessage);
    }
    if (value.apiMessage) {
      return value.apiMessage;
    }
  }
  return value instanceof Error ? value.message : 'Unexpected workflow operation error';
}

function appendDetail(message: string, detail: string | undefined): string {
  if (!detail || message.includes(detail)) {
    return message;
  }
  return `${message} ${detail}`;
}
