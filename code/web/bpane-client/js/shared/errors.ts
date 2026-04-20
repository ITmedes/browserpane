export abstract class BpaneError extends Error {
  readonly code: string;

  protected constructor(code: string, message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = new.target.name;
    this.code = code;
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

export class ValidationError extends BpaneError {
  constructor(code: string, message: string, options?: ErrorOptions) {
    super(code, message, options);
  }
}

export class UnsupportedFeatureError extends BpaneError {
  constructor(code: string, message: string, options?: ErrorOptions) {
    super(code, message, options);
  }
}

export class TransportError extends BpaneError {
  constructor(code: string, message: string, options?: ErrorOptions) {
    super(code, message, options);
  }
}
