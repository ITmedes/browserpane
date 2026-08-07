export type AdminEventStreamAccess = {
  readonly token: string;
  readonly endpointPath: string;
  readonly authenticationMessageType: string;
  readonly authenticatedMessageType: string;
};

export class AdminEventStreamAccessMapper {
  static fromResponse(value: unknown): AdminEventStreamAccess {
    if (!value || typeof value !== 'object') {
      throw new Error('admin event access-token response must be an object');
    }
    const response = value as Record<string, unknown>;
    const websocket = response.websocket;
    if (!websocket || typeof websocket !== 'object') {
      throw new Error('admin event access-token response is missing websocket metadata');
    }
    const descriptor = websocket as Record<string, unknown>;
    if (
      response.token_type !== 'admin_event_access_token'
      || typeof response.token !== 'string'
      || response.token.length === 0
      || descriptor.auth_type !== 'initial_message'
      || descriptor.endpoint_path !== '/api/v1/admin/events'
      || descriptor.authentication_message_type !== 'admin.authenticate'
      || descriptor.authenticated_message_type !== 'admin.authenticated'
    ) {
      throw new Error('admin event access-token response does not match the expected contract');
    }
    return {
      token: response.token,
      endpointPath: descriptor.endpoint_path,
      authenticationMessageType: descriptor.authentication_message_type,
      authenticatedMessageType: descriptor.authenticated_message_type,
    };
  }

  static toWebSocketUrl(baseUrl: string | URL, endpointPath = '/api/v1/admin/events'): string {
    const url = new URL(endpointPath, baseUrl);
    url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
    return url.toString();
  }
}
