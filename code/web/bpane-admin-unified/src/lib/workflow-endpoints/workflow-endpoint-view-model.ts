import { labelsToFormText, parseKeyValueLabels } from '$lib/application/admin-form-utils';
import type {
  UpsertWorkflowEndpointGrantRequest,
  UpsertWorkflowEndpointRequest,
  WorkflowEndpointGrantOperation,
  WorkflowEndpointResource,
} from './workflow-endpoint-types';

export type WorkflowEndpointDraft = {
  endpointKey: string;
  purpose: string;
  workflowDefinitionId: string;
  workflowDefinitionVersionId: string;
  workflowVersion: string;
  inputSchemaText: string;
  outputSchemaText: string;
  executionTimeoutSeconds: string;
  inlineResultMaxBytes: string;
  artifactRetentionSeconds: string;
  labelsText: string;
};

export type WorkflowEndpointGrantDraft = {
  servicePrincipalId: string;
  operations: WorkflowEndpointGrantOperation[];
};

export type WorkflowEndpointValidation<T> = {
  readonly valid: boolean;
  readonly request: T | null;
  readonly fieldErrors: Readonly<Record<string, readonly string[]>>;
};

export function createWorkflowEndpointDraft(
  endpoint?: WorkflowEndpointResource,
): WorkflowEndpointDraft {
  return {
    endpointKey: endpoint?.endpoint_key ?? '',
    purpose: endpoint?.purpose ?? '',
    workflowDefinitionId: endpoint?.workflow_definition_id ?? '',
    workflowDefinitionVersionId: endpoint?.workflow_definition_version_id ?? '',
    workflowVersion: endpoint?.workflow_version ?? '',
    inputSchemaText: JSON.stringify(endpoint?.input_schema ?? defaultInputSchema(), null, 2),
    outputSchemaText: JSON.stringify(endpoint?.output_schema ?? defaultOutputSchema(), null, 2),
    executionTimeoutSeconds: String(endpoint?.execution_timeout_seconds ?? 900),
    inlineResultMaxBytes: String(endpoint?.inline_result_max_bytes ?? 65_536),
    artifactRetentionSeconds: String(endpoint?.artifact_behavior.retention_seconds ?? 86_400),
    labelsText: labelsToFormText(endpoint?.labels ?? {}),
  };
}

export function validateWorkflowEndpointDraft(
  draft: WorkflowEndpointDraft,
): WorkflowEndpointValidation<UpsertWorkflowEndpointRequest> {
  const fieldErrors: Record<string, string[]> = {};
  const endpointKey = draft.endpointKey.trim();
  if (!/^[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?$/.test(endpointKey)) {
    fieldErrors.endpointKey = [
      'Use 1-64 lowercase letters or digits, with hyphens only inside the key.',
    ];
  }
  const purpose = draft.purpose.trim();
  if (!purpose || purpose.length > 512) {
    fieldErrors.purpose = ['Purpose must contain 1-512 characters.'];
  }
  for (const [field, value, label] of [
    ['workflowDefinitionId', draft.workflowDefinitionId, 'Workflow definition id'],
    ['workflowDefinitionVersionId', draft.workflowDefinitionVersionId, 'Workflow version id'],
    ['workflowVersion', draft.workflowVersion, 'Workflow version'],
  ] as const) {
    if (!value.trim()) fieldErrors[field] = [`${label} is required.`];
  }
  const inputSchema = parseSchema(draft.inputSchemaText, 'inputSchemaText', fieldErrors);
  const outputSchema = parseSchema(draft.outputSchemaText, 'outputSchemaText', fieldErrors);
  const executionTimeoutSeconds = boundedInteger(
    draft.executionTimeoutSeconds,
    1,
    86_400,
    'executionTimeoutSeconds',
    'Execution timeout',
    fieldErrors,
  );
  const inlineResultMaxBytes = boundedInteger(
    draft.inlineResultMaxBytes,
    1,
    1_048_576,
    'inlineResultMaxBytes',
    'Inline result limit',
    fieldErrors,
  );
  const artifactRetentionSeconds = boundedInteger(
    draft.artifactRetentionSeconds,
    1,
    2_592_000,
    'artifactRetentionSeconds',
    'Artifact retention',
    fieldErrors,
  );
  const labels = parseKeyValueLabels(draft.labelsText);
  if (!labels.ok) fieldErrors.labelsText = [labels.error];
  const valid = Object.keys(fieldErrors).length === 0;
  return {
    valid,
    fieldErrors,
    request: valid
      ? {
          endpoint_key: endpointKey,
          purpose,
          workflow_definition_id: draft.workflowDefinitionId.trim(),
          workflow_definition_version_id: draft.workflowDefinitionVersionId.trim(),
          workflow_version: draft.workflowVersion.trim(),
          input_schema: inputSchema,
          output_schema: outputSchema,
          execution_timeout_seconds: executionTimeoutSeconds,
          inline_result_max_bytes: inlineResultMaxBytes,
          artifact_behavior: {
            mode: 'authorized_references',
            retention_seconds: artifactRetentionSeconds,
          },
          labels: labels.ok ? labels.value : {},
        }
      : null,
  };
}

export function createWorkflowEndpointGrantDraft(): WorkflowEndpointGrantDraft {
  return {
    servicePrincipalId: '',
    operations: ['invoke', 'read'],
  };
}

export function validateWorkflowEndpointGrantDraft(
  draft: WorkflowEndpointGrantDraft,
): WorkflowEndpointValidation<UpsertWorkflowEndpointGrantRequest> {
  const fieldErrors: Record<string, string[]> = {};
  const servicePrincipalId = draft.servicePrincipalId.trim();
  if (!servicePrincipalId) {
    fieldErrors.servicePrincipalId = ['Service principal id is required.'];
  }
  const operations = [...new Set(draft.operations)];
  if (operations.length === 0) {
    fieldErrors.operations = ['Select at least one narrow endpoint operation.'];
  }
  const valid = Object.keys(fieldErrors).length === 0;
  return {
    valid,
    fieldErrors,
    request: valid ? { service_principal_id: servicePrincipalId, operations } : null,
  };
}

export function buildWorkflowEndpointInvocationExample(
  endpoint: WorkflowEndpointResource,
): string {
  const body = JSON.stringify(
    {
      input: exampleFromSchema(endpoint.input_schema),
      source_system: 'external-bpm',
      source_reference: 'process-instance-123',
    },
    null,
    2,
  );
  return [
    `curl --fail-with-body --request POST \\`,
    `  "/api/v1/projects/${endpoint.project_id}/workflow-endpoints/${endpoint.endpoint_key}/invocations" \\`,
    `  --header "Authorization: Bearer \$ACCESS_TOKEN" \\`,
    `  --header "Idempotency-Key: process-instance-123-activity-1" \\`,
    `  --header "Content-Type: application/json" \\`,
    `  --data '${body.replaceAll("'", "'\\''")}'`,
  ].join('\n');
}

function parseSchema(
  text: string,
  field: string,
  fieldErrors: Record<string, string[]>,
): unknown {
  try {
    const value = JSON.parse(text) as unknown;
    if (!value || typeof value !== 'object' || Array.isArray(value)) {
      fieldErrors[field] = ['Schema must be a JSON object.'];
    }
    return value;
  } catch {
    fieldErrors[field] = ['Schema must contain valid JSON.'];
    return {};
  }
}

function boundedInteger(
  text: string,
  minimum: number,
  maximum: number,
  field: string,
  label: string,
  fieldErrors: Record<string, string[]>,
): number {
  const value = Number(text);
  if (!Number.isInteger(value) || value < minimum || value > maximum) {
    fieldErrors[field] = [`${label} must be an integer from ${minimum} through ${maximum}.`];
  }
  return value;
}

function exampleFromSchema(schema: unknown): unknown {
  if (!schema || typeof schema !== 'object' || Array.isArray(schema)) return {};
  const object = schema as Record<string, unknown>;
  if (object.example !== undefined) return object.example;
  if (!object.properties || typeof object.properties !== 'object' || Array.isArray(object.properties)) {
    return {};
  }
  return Object.fromEntries(
    Object.entries(object.properties as Record<string, unknown>).map(([key, property]) => {
      const propertyObject = property && typeof property === 'object' && !Array.isArray(property)
        ? (property as Record<string, unknown>)
        : {};
      return [key, propertyObject.example ?? placeholderForType(propertyObject.type)];
    }),
  );
}

function placeholderForType(type: unknown): unknown {
  if (type === 'integer' || type === 'number') return 0;
  if (type === 'boolean') return false;
  if (type === 'array') return [];
  if (type === 'object') return {};
  return 'value';
}

function defaultInputSchema(): unknown {
  return {
    $schema: 'https://json-schema.org/draft/2020-12/schema',
    type: 'object',
    additionalProperties: false,
    properties: {},
  };
}

function defaultOutputSchema(): unknown {
  return defaultInputSchema();
}
