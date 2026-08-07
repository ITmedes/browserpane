export class ContractExampleValidator {
  constructor(openapi) {
    this.openapi = openapi;
  }

  validate(examples) {
    const errors = [];
    const names = new Set();
    for (const example of examples) {
      if (names.has(example.name)) errors.push(`${example.name}: duplicate example name`);
      names.add(example.name);
      try {
        this.#validateExample(example);
      } catch (error) {
        errors.push(`${example.name ?? 'unnamed'}: ${error.message}`);
      }
    }
    if (errors.length > 0) throw new Error(errors.join('\n'));
  }

  #validateExample(example) {
    const requestInput = {
      method: example.request?.method,
      path: example.request?.path,
      headers: { authorization: 'Bearer example-contract-token' }
    };
    if (example.request?.body !== undefined) requestInput.body = example.request.body;
    const [request, requestError] = this.openapi.request(requestInput);
    if (requestError) throw new Error(requestError.toString());
    if (request.operation.operationId !== example.operationId) {
      throw new Error(
        `resolved ${request.operation.operationId}, expected ${example.operationId}`
      );
    }
    const [, responseError] = request.response(
      example.response?.status,
      example.response?.body,
      { 'content-type': 'application/json' }
    );
    if (responseError) throw new Error(responseError.toString());
  }
}
