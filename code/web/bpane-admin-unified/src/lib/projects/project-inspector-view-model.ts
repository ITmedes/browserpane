import type { ProjectTone } from './project-formatters';
import type { ProjectResource } from './project-types';

export type ProjectInspectorModel = {
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly state: string;
  readonly stateTone: ProjectTone;
};

export function buildProjectInspectorModel(project: ProjectResource): ProjectInspectorModel {
  return {
    id: project.id,
    name: project.name,
    description: project.description?.trim() || 'No description.',
    state: project.state,
    stateTone: project.state === 'active' ? 'success' : 'neutral',
  };
}
