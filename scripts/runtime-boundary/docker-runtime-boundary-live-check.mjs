const ALLOWED_REQUESTS = Object.freeze([
  ["GET", "/_ping"],
  ["GET", "/version"],
  ["GET", "/info"],
  ["GET", "/containers/json?all=1"],
  ["GET", "/volumes"],
]);

const DENIED_REQUESTS = Object.freeze([
  ["POST", "/auth"],
  ["POST", "/build"],
  ["POST", "/commit"],
  ["GET", "/configs"],
  ["GET", "/distribution/example/json"],
  ["GET", "/events"],
  ["GET", "/exec/example/json"],
  ["GET", "/grpc"],
  ["GET", "/images/json"],
  ["GET", "/networks"],
  ["GET", "/nodes"],
  ["GET", "/plugins"],
  ["GET", "/secrets"],
  ["GET", "/services"],
  ["POST", "/session"],
  ["GET", "/swarm"],
  ["GET", "/system/df"],
  ["GET", "/tasks"],
]);

export class DockerRuntimeBoundaryLiveCheck {
  constructor(request) {
    this.request = request;
  }

  validate() {
    this.validateRequests(ALLOWED_REQUESTS, "200", "allowed");
    this.validateRequests(DENIED_REQUESTS, "403", "denied");
  }

  validateRequests(requests, expectedStatus, classification) {
    for (const [method, endpoint] of requests) {
      const status = this.request(method, endpoint);
      if (status !== expectedStatus) {
        throw new Error(
          `${classification} ${method} ${endpoint} must return ${expectedStatus}, received ${status}`,
        );
      }
    }
  }
}
