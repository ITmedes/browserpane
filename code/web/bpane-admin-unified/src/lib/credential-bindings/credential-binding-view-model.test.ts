import { describe, expect, it } from 'vitest';

import {
  buildCredentialBindingOverviewModel,
  credentialBindingMatchesSearch,
} from './credential-binding-view-model';
import {
  createCredentialBindingDraft,
  validateCredentialBindingDraft,
} from './credential-binding-form-model';
import type { CredentialBindingResource } from './credential-binding-types';

describe('credential binding view model', () => {
  it('summarizes scope and injection modes', () => {
    const model = buildCredentialBindingOverviewModel([
      binding({ id: 'owner', project: false, mode: 'form_fill' }),
      binding({ id: 'project', project: true, mode: 'totp_fill' }),
    ]);
    expect(model.metrics.map((metric) => [metric.label, metric.value])).toEqual([
      ['Bindings', '2'],
      ['Owner scoped', '1'],
      ['Project scoped', '1'],
      ['TOTP', '1'],
    ]);
    expect(model.rows[1]).toMatchObject({ scope: 'Support', injectionMode: 'TOTP fill' });
    expect(credentialBindingMatchesSearch(model.rows[0]!, 'support.example')).toBe(true);
  });

  it('builds write-only payload requests without retaining unrelated secret sources', () => {
    const draft = createCredentialBindingDraft();
    draft.name = 'Support login';
    draft.allowedOriginsText = 'https://support.example';
    draft.secretPayloadText = '{"fields":[{"selector":"#user","value":"demo"}]}';
    draft.labelsText = 'team=support';
    const validation = validateCredentialBindingDraft(draft);

    expect(validation).toMatchObject({ valid: true });
    expect(validation.request).toEqual({
      project_id: null,
      name: 'Support login',
      provider: 'vault_kv_v2',
      secret_payload: { fields: [{ selector: '#user', value: 'demo' }] },
      namespace: null,
      allowed_origins: ['https://support.example'],
      injection_mode: 'form_fill',
      totp: null,
      labels: { team: 'support' },
    });
    expect(validation.request).not.toHaveProperty('external_ref');
  });

  it('supports opaque existing references and project scope', () => {
    const draft = createCredentialBindingDraft();
    Object.assign(draft, {
      scope: 'project',
      projectId: 'project-1',
      name: 'Existing secret',
      secretSource: 'external_ref',
      externalRef: 'secret/data/existing',
    });
    const validation = validateCredentialBindingDraft(draft);
    expect(validation.request).toMatchObject({
      project_id: 'project-1',
      external_ref: 'secret/data/existing',
    });
    expect(validation.request).not.toHaveProperty('secret_payload');
  });

  it('rejects malformed secrets, origins, labels, project scope, and TOTP metadata', () => {
    const draft = createCredentialBindingDraft();
    Object.assign(draft, {
      scope: 'project',
      name: 'Broken',
      allowedOriginsText: 'not-a-url',
      secretPayloadText: '[]',
      labelsText: 'broken',
      injectionMode: 'totp_fill',
      totpPeriodSec: '0',
      totpDigits: 'nope',
    });
    expect(validateCredentialBindingDraft(draft)).toMatchObject({
      valid: false,
      fieldErrors: {
        projectId: ['Select a project.'],
        allowedOrigins: ['not-a-url must be an HTTP or HTTPS origin.'],
        labels: ['Labels must use key=value format.'],
        secretPayload: ['Secret payload must be a JSON object.'],
        totpPeriodSec: ['TOTP period must be a positive integer.'],
        totpDigits: ['TOTP digits must be a positive integer.'],
      },
    });
  });
});

function binding(options: {
  readonly id: string;
  readonly project: boolean;
  readonly mode: CredentialBindingResource['injection_mode'];
}): CredentialBindingResource {
  return {
    id: options.id,
    project_id: options.project ? 'project-1' : null,
    project: options.project ? { id: 'project-1', name: 'Support', state: 'active' } : null,
    name: options.id,
    provider: 'vault_kv_v2',
    external_ref: `secret/data/${options.id}`,
    namespace: null,
    allowed_origins: ['https://support.example'],
    injection_mode: options.mode,
    totp: options.mode === 'totp_fill' ? { period_sec: 30, digits: 6 } : null,
    labels: {},
    created_at: '2026-08-07T08:00:00.000Z',
    updated_at: '2026-08-07T09:00:00.000Z',
  };
}
