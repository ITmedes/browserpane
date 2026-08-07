import { describe, expect, it } from 'vitest';

import {
  buildExtensionOverviewModel,
  createExtensionDefinitionDraft,
  createExtensionVersionDraft,
  extensionMatchesSearch,
  validateExtensionDefinitionDraft,
  validateExtensionVersionDraft,
} from './extension-view-model';
import type { ExtensionDefinitionResource } from './extension-types';

describe('extension view model', () => {
  it('summarizes enablement and version state', () => {
    const model = buildExtensionOverviewModel([
      extension({ id: 'enabled', enabled: true, latestVersion: '1.0.0' }),
      extension({ id: 'disabled', enabled: false, latestVersion: null }),
    ]);

    expect(model.metrics.map((metric) => [metric.label, metric.value])).toEqual([
      ['Extensions', '2'],
      ['Enabled', '1'],
      ['Disabled', '1'],
      ['Versioned', '1'],
    ]);
    expect(model.rows[0]).toMatchObject({ status: 'Enabled', latestVersion: '1.0.0' });
    expect(model.rows[1]).toMatchObject({ status: 'Disabled', latestVersion: 'No version' });
  });

  it('searches extension metadata', () => {
    const [row] = buildExtensionOverviewModel([
      extension({ id: 'extension-1', enabled: true, latestVersion: '2.4.0' }),
    ]).rows;

    expect(extensionMatchesSearch(row!, 'platform')).toBe(true);
    expect(extensionMatchesSearch(row!, '2.4.0')).toBe(true);
    expect(extensionMatchesSearch(row!, 'missing')).toBe(false);
  });

  it('validates extension definition drafts', () => {
    expect(validateExtensionDefinitionDraft(createExtensionDefinitionDraft())).toMatchObject({
      valid: false,
      fieldErrors: { name: ['Name is required.'] },
    });
    expect(
      validateExtensionDefinitionDraft({
        name: ' Login helper ',
        description: ' Approved helper ',
        labelsText: 'team=platform',
      }),
    ).toEqual({
      valid: true,
      fieldErrors: {},
      request: {
        name: 'Login helper',
        description: 'Approved helper',
        labels: { team: 'platform' },
      },
    });
    expect(
      validateExtensionDefinitionDraft({
        name: 'Login helper',
        description: '',
        labelsText: 'invalid',
      }).fieldErrors.labels,
    ).toEqual(['Labels must use key=value format.']);
  });

  it('validates absolute installed paths for versions', () => {
    expect(validateExtensionVersionDraft(createExtensionVersionDraft())).toMatchObject({
      valid: false,
    });
    expect(
      validateExtensionVersionDraft({ version: '1.0.0', installPath: 'relative/path' }).fieldErrors,
    ).toEqual({ installPath: ['Install path must be absolute.'] });
    expect(
      validateExtensionVersionDraft({
        version: ' 1.0.0 ',
        installPath: ' /opt/extensions/login ',
      }).request,
    ).toEqual({ version: '1.0.0', install_path: '/opt/extensions/login' });
  });
});

function extension(options: {
  readonly id: string;
  readonly enabled: boolean;
  readonly latestVersion: string | null;
}): ExtensionDefinitionResource {
  return {
    id: options.id,
    name: `${options.id} extension`,
    description: 'Approved browser extension',
    enabled: options.enabled,
    latest_version: options.latestVersion,
    labels: { team: 'platform' },
    created_at: '2026-08-07T08:00:00.000Z',
    updated_at: '2026-08-07T09:00:00.000Z',
  };
}
