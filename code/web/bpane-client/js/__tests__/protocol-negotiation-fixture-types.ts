import type {
  ClientHello,
  NegotiationMessage,
  ProtocolFailureCode,
  ProtocolSupport,
  ServerSelection,
} from '../protocol.js';

export type NegotiationFixture = {
  readonly name: string;
  readonly direction: 'client_to_server' | 'server_to_client' | 'bidirectional';
  readonly wireHex: string;
  readonly expected:
    | { readonly outcome: 'valid'; readonly message: NegotiationMessage }
    | { readonly outcome: 'invalid'; readonly error: ProtocolFailureCode };
};

export type SelectionFixture = {
  readonly name: string;
  readonly operation: 'select' | 'validate_selection';
  readonly hello: ClientHello;
  readonly support: ProtocolSupport;
  readonly selection?: ServerSelection;
  readonly expected:
    | { readonly outcome: 'selected'; readonly selection: ServerSelection }
    | { readonly outcome: 'accepted' }
    | { readonly outcome: 'rejected'; readonly error: ProtocolFailureCode };
};
