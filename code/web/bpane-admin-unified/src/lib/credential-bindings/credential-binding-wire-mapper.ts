import type {
  CredentialBindingListResponse,
  CredentialBindingProjectOptionsResponse,
  CredentialBindingProjectResource,
  CredentialBindingResource,
  CredentialInjectionMode,
  CredentialTotpMetadata,
} from './credential-binding-types';
import { CredentialBindingCatalogError } from './credential-binding-error';

const INJECTION_MODES = [
  'form_fill',
  'cookie_seed',
  'storage_seed',
  'totp_fill',
] satisfies readonly CredentialInjectionMode[];
const PROJECT_STATES = [
  'active',
  'archived',
] satisfies readonly CredentialBindingProjectResource['state'][];

export class CredentialBindingWireMapper {
  static toListResponse(payload: unknown): CredentialBindingListResponse {
    const object = this.expectRecord(payload, 'credential binding list response');
    return {
      credential_bindings: this.expectArray(
        object.credential_bindings,
        'credential binding list resources',
      ).map((value) => this.toResource(value)),
    };
  }

  static toResource(value: unknown): CredentialBindingResource {
    const object = this.expectRecord(value, 'credential binding');
    return {
      id: this.expectString(object.id, 'credential binding id'),
      project_id: this.optionalString(object.project_id, 'credential binding project_id') ?? null,
      project: this.toNullableProject(object.project),
      name: this.expectString(object.name, 'credential binding name'),
      provider: this.expectEnum(
        object.provider,
        ['vault_kv_v2'] as const,
        'credential binding provider',
      ),
      external_ref: this.expectString(object.external_ref, 'credential binding external_ref'),
      namespace: this.optionalString(object.namespace, 'credential binding namespace') ?? null,
      allowed_origins: this.expectArray(
        object.allowed_origins,
        'credential binding allowed_origins',
      ).map((origin) => this.expectString(origin, 'credential binding allowed origin')),
      injection_mode: this.expectEnum(
        object.injection_mode,
        INJECTION_MODES,
        'credential binding injection_mode',
      ),
      totp: this.toNullableTotp(object.totp),
      labels: this.toStringRecord(object.labels, 'credential binding labels'),
      created_at: this.expectString(object.created_at, 'credential binding created_at'),
      updated_at: this.expectString(object.updated_at, 'credential binding updated_at'),
    };
  }

  static toProjectOptions(payload: unknown): CredentialBindingProjectOptionsResponse {
    const object = this.expectRecord(payload, 'project list response');
    return {
      projects: this.expectArray(object.projects, 'project list projects').map((value) =>
        this.toProject(value),
      ),
    };
  }

  private static toNullableProject(value: unknown): CredentialBindingProjectResource | null {
    return value === undefined || value === null ? null : this.toProject(value);
  }

  private static toProject(value: unknown): CredentialBindingProjectResource {
    const object = this.expectRecord(value, 'credential binding project');
    return {
      id: this.expectString(object.id, 'credential binding project id'),
      name: this.expectString(object.name, 'credential binding project name'),
      state: this.expectEnum(object.state, PROJECT_STATES, 'credential binding project state'),
    };
  }

  private static toNullableTotp(value: unknown): CredentialTotpMetadata | null {
    if (value === undefined || value === null) return null;
    const object = this.expectRecord(value, 'credential binding totp');
    return {
      issuer: this.optionalString(object.issuer, 'credential binding totp issuer') ?? null,
      account_name:
        this.optionalString(object.account_name, 'credential binding totp account_name') ?? null,
      period_sec:
        this.optionalNumber(object.period_sec, 'credential binding totp period_sec') ?? null,
      digits: this.optionalNumber(object.digits, 'credential binding totp digits') ?? null,
    };
  }

  private static expectRecord(value: unknown, label: string): Record<string, unknown> {
    if (!value || typeof value !== 'object' || Array.isArray(value)) {
      throw new CredentialBindingCatalogError(`${label} must be an object.`, 'invalid_payload');
    }
    return value as Record<string, unknown>;
  }

  private static expectArray(value: unknown, label: string): readonly unknown[] {
    if (!Array.isArray(value)) {
      throw new CredentialBindingCatalogError(`${label} must be an array.`, 'invalid_payload');
    }
    return value;
  }

  private static expectString(value: unknown, label: string): string {
    if (typeof value !== 'string') {
      throw new CredentialBindingCatalogError(`${label} must be a string.`, 'invalid_payload');
    }
    return value;
  }

  private static optionalString(value: unknown, label: string): string | null | undefined {
    return value === undefined || value === null ? value : this.expectString(value, label);
  }

  private static optionalNumber(value: unknown, label: string): number | null | undefined {
    if (value === undefined || value === null) return value;
    if (typeof value !== 'number' || !Number.isFinite(value)) {
      throw new CredentialBindingCatalogError(
        `${label} must be a finite number.`,
        'invalid_payload',
      );
    }
    return value;
  }

  private static expectEnum<T extends string>(
    value: unknown,
    allowed: readonly T[],
    label: string,
  ): T {
    const candidate = this.expectString(value, label);
    if (!allowed.includes(candidate as T)) {
      throw new CredentialBindingCatalogError(
        `${label} must be one of ${allowed.join(', ')}.`,
        'invalid_payload',
      );
    }
    return candidate as T;
  }

  private static toStringRecord(value: unknown, label: string): Readonly<Record<string, string>> {
    const object = this.expectRecord(value, label);
    return Object.fromEntries(
      Object.entries(object).map(([key, item]) => [
        key,
        this.expectString(item, `${label}.${key}`),
      ]),
    );
  }
}
