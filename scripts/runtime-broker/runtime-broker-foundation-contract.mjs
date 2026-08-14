import { RuntimeBrokerServiceSecurityContract } from "./runtime-broker-service-security-contract.mjs";

const REQUIRED_BROKER_NETWORKS = ["runtime-broker-api", "runtime-broker-auth"];

export class RuntimeBrokerFoundationContract {
  validate(config) {
    const services = config.services ?? {};
    const networks = config.networks ?? {};
    const broker = services["runtime-broker"];
    const gateway = services.gateway;
    const keycloak = services.keycloak;

    this.expect(broker, "compose service runtime-broker is missing");
    this.expect(gateway, "compose service gateway is missing");
    this.expect(keycloak, "compose service keycloak is missing");
    new RuntimeBrokerServiceSecurityContract().validate(broker);
    for (const network of REQUIRED_BROKER_NETWORKS) {
      this.expect(networks[network]?.internal === true, `${network} must be internal`);
    }

    this.sameValues(
      Object.keys(broker.networks ?? {}),
      REQUIRED_BROKER_NETWORKS,
      "runtime-broker must join only its API and auth networks",
    );
    this.sameValues(
      this.networkMembers(services, "runtime-broker-api"),
      ["gateway", "runtime-broker"],
      "runtime-broker-api must contain only gateway and runtime-broker",
    );
    this.sameValues(
      this.networkMembers(services, "runtime-broker-auth"),
      ["keycloak", "runtime-broker"],
      "runtime-broker-auth must contain only keycloak and runtime-broker",
    );
    this.expect(
      !Object.keys(broker.networks ?? {}).includes("docker-control"),
      "runtime-broker foundation must not join docker-control",
    );
    const command = Array.isArray(broker.command)
      ? broker.command.join(" ")
      : String(broker.command ?? "");
    for (const required of [
      "--oidc-audience bpane-runtime-broker",
      "--allowed-client-id bpane-runtime-broker-gateway",
      "--oidc-jwks-url http://keycloak:8080/",
    ]) {
      this.expect(command.includes(required), `runtime-broker command must contain ${required}`);
    }
    this.expect(
      !command.toLowerCase().includes("secret"),
      "runtime-broker command must not contain a client secret",
    );
  }

  networkMembers(services, network) {
    return Object.entries(services)
      .filter(([, service]) => Object.keys(service.networks ?? {}).includes(network))
      .map(([name]) => name)
      .sort();
  }

  sameValues(actual, expected, message) {
    this.expect(
      JSON.stringify([...actual].sort()) === JSON.stringify([...expected].sort()),
      message,
    );
  }

  expect(condition, message) {
    if (!condition) {
      throw new Error(message);
    }
  }
}
