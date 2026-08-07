export type ExtensionDefinitionResource = {
  readonly id: string;
  readonly name: string;
  readonly description?: string | null;
  readonly enabled: boolean;
  readonly latest_version?: string | null;
  readonly labels: Readonly<Record<string, string>>;
  readonly created_at: string;
  readonly updated_at: string;
};

export type ExtensionVersionResource = {
  readonly id: string;
  readonly extension_definition_id: string;
  readonly version: string;
  readonly install_path: string;
  readonly created_at: string;
};

export type ExtensionDefinitionListResponse = {
  readonly extensions: readonly ExtensionDefinitionResource[];
};

export type CreateExtensionDefinitionRequest = {
  readonly name: string;
  readonly description?: string | null;
  readonly labels?: Readonly<Record<string, string>>;
};

export type CreateExtensionVersionRequest = {
  readonly version: string;
  readonly install_path: string;
};
