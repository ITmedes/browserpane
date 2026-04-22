import { ValidationError } from './errors.js';

interface ConnectOptionsLike {
  container?: unknown;
  gatewayUrl?: unknown;
  accessToken?: unknown;
  token?: unknown;
}

export class SessionConnectOptionsValidator {
  static validate(options: ConnectOptionsLike | null | undefined): void {
    if (!options || !(options.container instanceof HTMLElement)) {
      throw new ValidationError(
        'bpane.connect.invalid_container',
        'BpaneSession.connect requires a valid container HTMLElement',
      );
    }
    if (typeof options.gatewayUrl !== 'string' || options.gatewayUrl.trim() === '') {
      throw new ValidationError(
        'bpane.connect.invalid_gateway_url',
        'BpaneSession.connect requires a non-empty gatewayUrl',
      );
    }
    const accessToken = typeof options.accessToken === 'string'
      ? options.accessToken
      : typeof options.token === 'string'
        ? options.token
        : null;
    if (typeof accessToken !== 'string' || accessToken.trim() === '') {
      throw new ValidationError(
        'bpane.connect.invalid_access_token',
        'BpaneSession.connect requires a non-empty accessToken',
      );
    }
  }
}
