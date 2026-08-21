import {
  ProtocolNegotiationError,
  ProtocolNegotiationRegistry,
  type ClientHello,
  type ProtocolCapability,
  type ProtocolSupport,
  type ServerSelection,
} from './protocol-negotiation-types.js';
import { ProtocolNegotiationValidator } from './protocol-negotiation-validator.js';

/** Pure highest-common-version and capability selection. */
export class ProtocolNegotiator {
  public select(hello: ClientHello, support: ProtocolSupport): ServerSelection {
    ProtocolNegotiationValidator.assertClientHello(hello);
    ProtocolNegotiationValidator.assertSupport(support);
    const selectedVersion = [...support.versions]
      .reverse()
      .find((version) => hello.versions.includes(version));
    if (selectedVersion === undefined) {
      throw new ProtocolNegotiationError('unsupported_protocol_version');
    }

    for (const id of hello.requiredCapabilities) {
      if (!ProtocolNegotiationRegistry.isKnownCapability(id)
        || !support.capabilities.includes(id)
        || !this.hasRequiredDependency(hello, support, id)) {
        throw new ProtocolNegotiationError('required_protocol_capability_missing');
      }
    }

    const offered = [...hello.requiredCapabilities, ...hello.optionalCapabilities];
    const selected = offered
      .filter(ProtocolNegotiationRegistry.isKnownCapability)
      .filter((capability) => support.capabilities.includes(capability))
      .sort((left, right) => left - right)
      .filter((capability, index, values) => index === 0 || values[index - 1] !== capability);
    const withValidDependencies = selected.filter((capability) => {
      const dependency = ProtocolNegotiationRegistry.dependency(capability);
      return dependency === undefined || selected.includes(dependency);
    });
    const selection = { selectedVersion, capabilities: withValidDependencies };
    ProtocolNegotiationValidator.assertServerSelection(selection);
    return selection;
  }

  public validateSelection(
    hello: ClientHello,
    support: ProtocolSupport,
    selection: ServerSelection,
  ): void {
    ProtocolNegotiationValidator.assertServerSelection(selection);
    const expected = this.select(hello, support);
    const selectedIsCommon = hello.versions.includes(selection.selectedVersion)
      && support.versions.includes(selection.selectedVersion);
    if (selectedIsCommon && selection.selectedVersion < expected.selectedVersion) {
      throw new ProtocolNegotiationError('protocol_downgrade_refused');
    }
    if (selection.selectedVersion !== expected.selectedVersion
      || !this.equalCapabilities(selection.capabilities, expected.capabilities)) {
      throw new ProtocolNegotiationError('protocol_selection_mismatch');
    }
  }

  private hasRequiredDependency(
    hello: ClientHello,
    support: ProtocolSupport,
    capability: ProtocolCapability,
  ): boolean {
    const dependency = ProtocolNegotiationRegistry.dependency(capability);
    return dependency === undefined
      || (
        support.capabilities.includes(dependency)
        && (
          hello.requiredCapabilities.includes(dependency)
          || hello.optionalCapabilities.includes(dependency)
        )
      );
  }

  private equalCapabilities(left: readonly number[], right: readonly number[]): boolean {
    return left.length === right.length
      && left.every((capability, index) => right[index] === capability);
  }
}
