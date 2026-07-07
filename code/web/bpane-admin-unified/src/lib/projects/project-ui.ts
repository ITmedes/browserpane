import type { ProjectTone } from './project-formatters';

export function projectToneClass(tone: ProjectTone): string {
  if (tone === 'success') {
    return 'bg-emerald-50 text-emerald-700 ring-emerald-200';
  }
  if (tone === 'warning') {
    return 'bg-amber-50 text-amber-700 ring-amber-200';
  }
  if (tone === 'danger') {
    return 'bg-red-50 text-red-700 ring-red-200';
  }
  return 'bg-slate-100 text-slate-600 ring-slate-200';
}
