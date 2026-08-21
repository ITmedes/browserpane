import {
  ProtocolNegotiationError,
  ProtocolNegotiationRegistry,
  ProtocolSupportError,
  type ClientHello,
  type ProtocolCapability,
  type ProtocolSupport,
  type ServerSelection,
} from './protocol-negotiation-types.js';

export const MAX_PROTOCOL_VERSIONS = 8;
export const MAX_PROTOCOL_CAPABILITIES = 64;

/** Shared structural validation for codecs and pure negotiation. */
export class ProtocolNegotiationValidator {
  public static assertClientHello(hello: ClientHello): void {
    const valid = hello.versions.length >= 1
      && hello.versions.length <= MAX_PROTOCOL_VERSIONS
      && hello.requiredCapabilities.length <= MAX_PROTOCOL_CAPABILITIES
      && hello.optionalCapabilities.length <= MAX_PROTOCOL_CAPABILITIES
      && hello.requiredCapabilities.length + hello.optionalCapabilities.length
        <= MAX_PROTOCOL_CAPABILITIES
      && hello.versions.every((version) => this.isUint16(version) && version !== 0)
      && hello.requiredCapabilities.every((id) => this.isUint16(id))
      && hello.optionalCapabilities.every((id) => this.isUint16(id))
      && this.isCanonical(hello.versions)
      && this.isCanonical(hello.requiredCapabilities)
      && this.isCanonical(hello.optionalCapabilities)
      && hello.requiredCapabilities.every(
        (id) => !hello.optionalCapabilities.includes(id),
      );
    if (!valid) {
      throw new ProtocolNegotiationError('malformed_protocol_hello');
    }
  }

  public static assertServerSelection(selection: ServerSelection): void {
    const structureValid = this.isUint16(selection.selectedVersion)
      && selection.selectedVersion !== 0
      && selection.capabilities.length <= MAX_PROTOCOL_CAPABILITIES
      && selection.capabilities.every((id) => this.isUint16(id))
      && this.isCanonical(selection.capabilities);
    const valuesValid = structureValid
      && selection.capabilities.every((id) => {
        if (!ProtocolNegotiationRegistry.isKnownCapability(id)) {
          return false;
        }
        const capability = id;
        const dependency = ProtocolNegotiationRegistry.dependency(capability);
        return dependency === undefined || selection.capabilities.includes(dependency);
      });
    const audioCount = valuesValid
      ? selection.capabilities.filter((id) => {
        return ProtocolNegotiationRegistry.isKnownCapability(id)
          && ProtocolNegotiationRegistry.isDesktopAudio(id);
      }).length
      : 0;
    const valid = valuesValid && audioCount <= 1;
    if (!valid) {
      throw new ProtocolNegotiationError('protocol_selection_mismatch');
    }
  }

  public static assertSupport(support: ProtocolSupport): void {
    if (support.versions.length < 1
      || support.versions.length > MAX_PROTOCOL_VERSIONS
      || !support.versions.every((version) => this.isUint16(version) && version !== 0)
      || !this.isCanonical(support.versions)) {
      throw new ProtocolSupportError('versions_not_canonical');
    }
    if (support.capabilities.length > MAX_PROTOCOL_CAPABILITIES
      || !this.isCanonical(support.capabilities)
      || !support.capabilities.every(ProtocolNegotiationRegistry.isKnownCapability)) {
      throw new ProtocolSupportError('capabilities_not_canonical');
    }
    if (support.capabilities.filter(ProtocolNegotiationRegistry.isDesktopAudio).length > 1) {
      throw new ProtocolSupportError('multiple_desktop_audio_codecs');
    }
    if (support.capabilities.some((capability) => {
      const dependency = ProtocolNegotiationRegistry.dependency(capability);
      return dependency !== undefined && !support.capabilities.includes(dependency);
    })) {
      throw new ProtocolSupportError('capability_dependency_missing');
    }
  }

  private static isCanonical(values: readonly number[]): boolean {
    return values.every((value, index) => index === 0 || values[index - 1] < value);
  }

  private static isUint16(value: number): boolean {
    return Number.isInteger(value) && value >= 0 && value <= 0xFFFF;
  }
}
