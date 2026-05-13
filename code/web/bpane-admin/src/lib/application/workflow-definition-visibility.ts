import type { WorkflowDefinitionResource } from '../api/workflow-types';

export const ADMIN_TEMPLATE_LABEL = 'bpane_admin_template';
export const ADMIN_HIDDEN_LABEL = 'bpane_admin_hidden';
export const BROWSERPANE_TOUR_TEMPLATE = 'browserpane-tour';
export const INCLUDE_HIDDEN_STORAGE_KEY = 'bpane.admin.showHiddenWorkflowDefinitions';

export function visibleWorkflowDefinitions(
  definitions: readonly WorkflowDefinitionResource[],
): readonly WorkflowDefinitionResource[] {
  const visible = definitions.filter(isVisibleWorkflowDefinition);
  const templates = visible.filter(isAdminTemplateDefinition);
  const others = visible.filter((definition) => !isAdminTemplateDefinition(definition));
  return [...templates.sort(compareByName), ...others.sort(compareByName)];
}

export function hiddenWorkflowDefinitions(
  definitions: readonly WorkflowDefinitionResource[],
): readonly WorkflowDefinitionResource[] {
  return definitions.filter((definition) => !isVisibleWorkflowDefinition(definition));
}

export function isVisibleWorkflowDefinition(definition: WorkflowDefinitionResource): boolean {
  if (definition.labels[ADMIN_HIDDEN_LABEL] === 'true') {
    return false;
  }
  if (isAdminTemplateDefinition(definition)) {
    return Boolean(definition.latest_version);
  }
  const suite = definition.labels.suite ?? '';
  return !suite.toLowerCase().includes('smoke') && !definition.name.toLowerCase().includes('smoke');
}

export function isBrowserPaneTourDefinition(definition: WorkflowDefinitionResource): boolean {
  return definition.labels[ADMIN_TEMPLATE_LABEL] === BROWSERPANE_TOUR_TEMPLATE;
}

export function isAdminTemplateDefinition(definition: WorkflowDefinitionResource): boolean {
  return Boolean(definition.labels[ADMIN_TEMPLATE_LABEL]);
}

export function workflowDefinitionKind(definition: WorkflowDefinitionResource): string {
  if (isBrowserPaneTourDefinition(definition)) {
    return 'Example template';
  }
  if (isAdminTemplateDefinition(definition)) {
    return 'Template';
  }
  return 'Workflow';
}

export function includeHiddenWorkflowDefinitions(): boolean {
  try {
    return globalThis.sessionStorage?.getItem(INCLUDE_HIDDEN_STORAGE_KEY) === 'true';
  } catch {
    return false;
  }
}

function compareByName(left: WorkflowDefinitionResource, right: WorkflowDefinitionResource): number {
  return left.name.localeCompare(right.name);
}
