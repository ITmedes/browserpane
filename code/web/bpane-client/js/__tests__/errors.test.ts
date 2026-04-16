import { describe, expect, it } from 'vitest';
import { SessionConnectOptionsValidator } from '../shared/connect-options-validator.js';
import { UnsupportedFeatureError, ValidationError } from '../shared/errors.js';

describe('shared errors', () => {
  it('preserves code and cause on ValidationError', () => {
    const cause = new Error('root cause');
    const error = new ValidationError(
      'bpane.test.invalid_value',
      'Invalid test value',
      { cause },
    );

    expect(error).toBeInstanceOf(Error);
    expect(error.name).toBe('ValidationError');
    expect(error.code).toBe('bpane.test.invalid_value');
    expect(error.message).toBe('Invalid test value');
    expect(error.cause).toBe(cause);
  });

  it('preserves code on UnsupportedFeatureError', () => {
    const error = new UnsupportedFeatureError(
      'bpane.test.unsupported',
      'Unsupported browser capability',
    );

    expect(error.name).toBe('UnsupportedFeatureError');
    expect(error.code).toBe('bpane.test.unsupported');
    expect(error.message).toBe('Unsupported browser capability');
  });
});

describe('SessionConnectOptionsValidator', () => {
  it('throws ValidationError for invalid container', () => {
    expect(() => SessionConnectOptionsValidator.validate({
      container: null,
      gatewayUrl: 'https://localhost:4433',
      token: 'test',
    })).toThrowError(ValidationError);

    try {
      SessionConnectOptionsValidator.validate({
        container: null,
        gatewayUrl: 'https://localhost:4433',
        token: 'test',
      });
    } catch (error) {
      expect(error).toMatchObject({
        code: 'bpane.connect.invalid_container',
        message: 'BpaneSession.connect requires a valid container HTMLElement',
      });
    }
  });

  it('throws ValidationError for empty gatewayUrl', () => {
    expect(() => SessionConnectOptionsValidator.validate({
      container: document.createElement('div'),
      gatewayUrl: '   ',
      token: 'test',
    })).toThrowError(ValidationError);
  });

  it('throws ValidationError for empty token', () => {
    expect(() => SessionConnectOptionsValidator.validate({
      container: document.createElement('div'),
      gatewayUrl: 'https://localhost:4433',
      token: '   ',
    })).toThrowError(ValidationError);
  });
});
