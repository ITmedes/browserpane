import {
  AdminApiRequestError,
  type AdminApiRequestErrorCode,
  type AdminApiRequestFailure,
} from '$lib/api/authenticated-api';

export class CredentialBindingCatalogError extends AdminApiRequestError {
  constructor(
    message: string,
    code: AdminApiRequestErrorCode,
    status: number | null = null,
    failure?: AdminApiRequestFailure,
  ) {
    super(message, failure ?? { code, status, message });
    this.name = 'CredentialBindingCatalogError';
  }
}
