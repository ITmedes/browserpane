import type {
  WorkflowDefinitionListResponse,
  WorkflowDefinitionResource,
  WorkflowDefinitionVersionListResponse,
  WorkflowDefinitionVersionResource,
  WorkflowSourceResource,
} from './workflow-types';
import {
  expectRecord,
  expectString,
  expectStringRecord,
  optionalString,
} from './control-wire';

export class WorkflowMapper {
  static toDefinitionList(payload: unknown): WorkflowDefinitionListResponse {
    const object = expectRecord(payload, 'workflow definition list response');
    return {
      workflows: expectArray(object.workflows, 'workflow definition list workflows')
        .map((workflow) => this.toDefinition(workflow)),
    };
  }

  static toDefinitionVersionList(payload: unknown): WorkflowDefinitionVersionListResponse {
    const object = expectRecord(payload, 'workflow definition version list response');
    return {
      versions: expectArray(object.versions, 'workflow definition version list versions')
        .map((version) => this.toDefinitionVersion(version)),
    };
  }

  static toDefinition(payload: unknown): WorkflowDefinitionResource {
    const object = expectRecord(payload, 'workflow definition resource');
    const description = optionalString(object.description, 'workflow definition description');
    const latestVersion = optionalString(
      object.latest_version,
      'workflow definition latest_version',
    );
    return {
      id: expectString(object.id, 'workflow definition id'),
      name: expectString(object.name, 'workflow definition name'),
      ...(description !== undefined ? { description } : {}),
      labels: expectStringRecord(object.labels ?? {}, 'workflow definition labels'),
      ...(latestVersion !== undefined ? { latest_version: latestVersion } : {}),
      created_at: expectString(object.created_at, 'workflow definition created_at'),
      updated_at: expectString(object.updated_at, 'workflow definition updated_at'),
    };
  }

  static toDefinitionVersion(payload: unknown): WorkflowDefinitionVersionResource {
    const object = expectRecord(payload, 'workflow definition version resource');
    return {
      id: expectString(object.id, 'workflow definition version id'),
      workflow_definition_id: expectString(
        object.workflow_definition_id,
        'workflow definition version workflow_definition_id',
      ),
      version: expectString(object.version, 'workflow definition version version'),
      executor: expectString(object.executor, 'workflow definition version executor'),
      entrypoint: expectString(object.entrypoint, 'workflow definition version entrypoint'),
      ...(object.source !== undefined ? { source: toWorkflowSource(object.source) } : {}),
      ...(object.input_schema !== undefined ? { input_schema: object.input_schema } : {}),
      ...(object.output_schema !== undefined ? { output_schema: object.output_schema } : {}),
      ...(object.default_session !== undefined ? { default_session: object.default_session } : {}),
      allowed_credential_binding_ids: expectStringArray(
        object.allowed_credential_binding_ids ?? [],
        'workflow definition version allowed_credential_binding_ids',
      ),
      allowed_extension_ids: expectStringArray(
        object.allowed_extension_ids ?? [],
        'workflow definition version allowed_extension_ids',
      ),
      allowed_file_workspace_ids: expectStringArray(
        object.allowed_file_workspace_ids ?? [],
        'workflow definition version allowed_file_workspace_ids',
      ),
      created_at: expectString(object.created_at, 'workflow definition version created_at'),
    };
  }

}

function toWorkflowSource(value: unknown): WorkflowSourceResource | null {
  if (value === null) {
    return null;
  }
  const object = expectRecord(value, 'workflow definition version source');
  const kind = expectString(object.kind, 'workflow definition version source kind');
  if (kind !== 'git') {
    throw new Error(`workflow definition version source kind ${kind} is not supported`);
  }
  const ref = optionalString(object.ref, 'workflow definition version source ref');
  const resolvedCommit = optionalString(
    object.resolved_commit,
    'workflow definition version source resolved_commit',
  );
  const rootPath = optionalString(object.root_path, 'workflow definition version source root_path');
  return {
    kind,
    repository_url: expectString(
      object.repository_url,
      'workflow definition version source repository_url',
    ),
    ...(ref !== undefined ? { ref } : {}),
    ...(resolvedCommit !== undefined ? { resolved_commit: resolvedCommit } : {}),
    ...(rootPath !== undefined ? { root_path: rootPath } : {}),
  };
}

function expectArray(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) {
    throw new Error(`${label} must be an array`);
  }
  return value;
}

function expectStringArray(value: unknown, label: string): readonly string[] {
  return expectArray(value, label).map((entry) => expectString(entry, `${label} entry`));
}
