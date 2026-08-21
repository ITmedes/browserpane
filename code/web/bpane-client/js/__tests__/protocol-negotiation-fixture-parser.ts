import {
  ProtocolNegotiationRegistry,
  type ClientHello,
  type NegotiationMessage,
  type ProtocolSupport,
  type ServerSelection,
} from '../protocol.js';
import type {
  NegotiationFixture,
  SelectionFixture,
} from './protocol-negotiation-fixture-types.js';

/** Strict parser for schema-v2 negotiation conformance vectors. */
export class ProtocolNegotiationFixtureParser {
  public parseNegotiation(input: unknown, index: number): NegotiationFixture {
    const record = this.record(input, `negotiation vector ${index}`);
    const name = this.string(record.name, 'negotiation vector name');
    const direction = record.direction;
    if (direction !== 'client_to_server'
      && direction !== 'server_to_client'
      && direction !== 'bidirectional') {
      throw new Error(`invalid negotiation direction for ${name}`);
    }
    return {
      name,
      direction,
      wireHex: this.string(record.wire_hex, `wire hex for ${name}`),
      expected: this.negotiationExpectation(record.expected, name),
    };
  }

  public parseSelection(input: unknown, index: number): SelectionFixture {
    const record = this.record(input, `selection vector ${index}`);
    const name = this.string(record.name, 'selection vector name');
    const operation = record.operation;
    if (operation !== 'select' && operation !== 'validate_selection') {
      throw new Error(`invalid selection operation for ${name}`);
    }
    const selection = record.selection === undefined
      ? undefined
      : this.selection(record.selection, name);
    if (operation === 'validate_selection' && selection === undefined) {
      throw new Error(`missing selection for ${name}`);
    }
    return {
      name,
      operation,
      hello: this.hello(record.hello, name),
      support: this.support(record.support, name),
      ...(selection === undefined ? {} : { selection }),
      expected: this.selectionExpectation(record.expected, name),
    };
  }

  private negotiationExpectation(
    input: unknown,
    name: string,
  ): NegotiationFixture['expected'] {
    const expected = this.record(input, `expectation for ${name}`);
    if (expected.outcome === 'invalid'
      && ProtocolNegotiationRegistry.isFailureCode(expected.error)) {
      return { outcome: 'invalid', error: expected.error };
    }
    if (expected.outcome !== 'valid') {
      throw new Error(`invalid negotiation outcome for ${name}`);
    }
    return {
      outcome: 'valid',
      message: this.expectedMessage(expected, name),
    };
  }

  private expectedMessage(
    expected: Record<string, unknown>,
    name: string,
  ): NegotiationMessage {
    if (expected.message === 'client_hello') {
      return { type: 'client_hello', hello: this.hello(expected, name) };
    }
    if (expected.message === 'server_selection') {
      return { type: 'server_selection', selection: this.selection(expected, name) };
    }
    if (expected.message === 'protocol_reject'
      && ProtocolNegotiationRegistry.isFailureCode(expected.failure)) {
      return { type: 'protocol_reject', failure: expected.failure };
    }
    throw new Error(`invalid negotiation message for ${name}`);
  }

  private selectionExpectation(
    input: unknown,
    name: string,
  ): SelectionFixture['expected'] {
    const expected = this.record(input, `selection expectation for ${name}`);
    if (expected.outcome === 'selected') {
      return { outcome: 'selected', selection: this.selection(expected, name) };
    }
    if (expected.outcome === 'accepted') {
      return { outcome: 'accepted' };
    }
    if (expected.outcome === 'rejected'
      && ProtocolNegotiationRegistry.isFailureCode(expected.error)) {
      return { outcome: 'rejected', error: expected.error };
    }
    throw new Error(`invalid selection outcome for ${name}`);
  }

  private hello(input: unknown, name: string): ClientHello {
    const record = this.record(input, `hello for ${name}`);
    return {
      versions: this.numberArray(record.versions, `versions for ${name}`),
      requiredCapabilities: this.numberArray(
        record.required_capabilities,
        `required capabilities for ${name}`,
      ),
      optionalCapabilities: this.numberArray(
        record.optional_capabilities,
        `optional capabilities for ${name}`,
      ),
    };
  }

  private support(input: unknown, name: string): ProtocolSupport {
    const record = this.record(input, `support for ${name}`);
    const capabilities = this.numberArray(record.capabilities, `support capabilities for ${name}`);
    if (!capabilities.every(ProtocolNegotiationRegistry.isKnownCapability)) {
      throw new Error(`unknown support capability for ${name}`);
    }
    return {
      versions: this.numberArray(record.versions, `support versions for ${name}`),
      capabilities,
    };
  }

  private selection(input: unknown, name: string): ServerSelection {
    const record = this.record(input, `selection for ${name}`);
    if (typeof record.selected_version !== 'number') {
      throw new Error(`invalid selected version for ${name}`);
    }
    return {
      selectedVersion: record.selected_version,
      capabilities: this.numberArray(record.capabilities, `selected capabilities for ${name}`),
    };
  }

  private numberArray(input: unknown, label: string): number[] {
    if (!Array.isArray(input) || !input.every((value) => typeof value === 'number')) {
      throw new Error(`invalid ${label}`);
    }
    return input;
  }

  private string(input: unknown, label: string): string {
    if (typeof input !== 'string') {
      throw new Error(`invalid ${label}`);
    }
    return input;
  }

  private record(input: unknown, label: string): Record<string, unknown> {
    if (typeof input !== 'object' || input === null || Array.isArray(input)) {
      throw new Error(`invalid ${label}`);
    }
    return Object.fromEntries(Object.entries(input));
  }
}
