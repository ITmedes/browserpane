export type CredentialBindingProvider = 'vault_kv_v2';
export type CredentialInjectionMode = 'form_fill' | 'cookie_seed' | 'storage_seed' | 'totp_fill';

export type CredentialBindingProjectResource = {
  readonly id: string;
  readonly name: string;
  readonly state: 'active' | 'archived';
};

export type CredentialTotpMetadata = {
  readonly issuer?: string | null;
  readonly account_name?: string | null;
  readonly period_sec?: number | null;
  readonly digits?: number | null;
};

export type CredentialBindingResource = {
  readonly id: string;
  readonly project_id?: string | null;
  readonly project?: CredentialBindingProjectResource | null;
  readonly name: string;
  readonly provider: CredentialBindingProvider;
  readonly external_ref: string;
  readonly namespace?: string | null;
  readonly allowed_origins: readonly string[];
  readonly injection_mode: CredentialInjectionMode;
  readonly totp?: CredentialTotpMetadata | null;
  readonly labels: Readonly<Record<string, string>>;
  readonly created_at: string;
  readonly updated_at: string;
};

export type CredentialBindingListResponse = {
  readonly credential_bindings: readonly CredentialBindingResource[];
};

export type CredentialBindingProjectOptionsResponse = {
  readonly projects: readonly CredentialBindingProjectResource[];
};

export type CreateCredentialBindingRequest = {
  readonly project_id?: string | null;
  readonly name: string;
  readonly provider: CredentialBindingProvider;
  readonly external_ref?: string | null;
  readonly namespace?: string | null;
  readonly allowed_origins: readonly string[];
  readonly injection_mode: CredentialInjectionMode;
  readonly totp?: CredentialTotpMetadata | null;
  readonly secret_payload?: unknown;
  readonly labels: Readonly<Record<string, string>>;
};
