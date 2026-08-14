import { RuntimeBrokerServiceSecurityContract } from "./runtime-broker-service-security-contract.mjs";

const BROKER_NETWORKS = ["docker-control", "runtime-broker-api", "runtime-broker-auth"];

export class RuntimeBrokerBrowserOverlayContract {
  validate(config) {
    const services = config?.services ?? {};
    const broker = services["runtime-broker"];
    const gateway = services.gateway;
    const proxy = services["docker-proxy"];
    this.expect(broker, "compose service runtime-broker is missing");
    this.expect(gateway, "compose service gateway is missing");
    this.expect(proxy, "compose service docker-proxy is missing");
    new RuntimeBrokerServiceSecurityContract().validate(broker);

    this.sameValues(
      Object.keys(broker.networks ?? {}),
      BROKER_NETWORKS,
      "runtime-broker overlay networks are invalid",
    );
    this.sameValues(
      this.networkMembers(services, "docker-control"),
      ["docker-proxy", "runtime-broker"],
      "docker-control must contain only the proxy and runtime-broker",
    );
    this.expect(
      broker.environment?.BPANE_RUNTIME_BROKER_EXECUTOR === "docker-browser",
      "runtime-broker must select the docker-browser executor",
    );
    this.expect(
      broker.environment?.BPANE_RUNTIME_BROKER_DOCKER_API_URL ===
        "http://docker-proxy:2375",
      "runtime-broker must use the internal Docker proxy",
    );
    this.expect(
      this.isImmutableImage(broker.environment?.BPANE_RUNTIME_BROKER_BROWSER_IMAGE),
      "runtime-broker browser image must be immutable",
    );
    this.expect(
      this.isImmutableImage(broker.environment?.BPANE_RUNTIME_BROKER_WORKFLOW_IMAGE),
      "runtime-broker workflow image must be immutable",
    );
    this.expect(
      this.isImmutableImage(broker.environment?.BPANE_RUNTIME_BROKER_RECORDING_IMAGE),
      "runtime-broker recording image must be immutable",
    );
    this.expect(
      this.isImmutableImage(
        broker.environment?.BPANE_RUNTIME_BROKER_STORAGE_HELPER_IMAGE,
      ),
      "runtime-broker storage helper image must be immutable",
    );
    this.expect(
      broker.environment?.BPANE_RUNTIME_BROKER_STORAGE_HELPER_IMAGE ===
        broker.environment?.BPANE_RUNTIME_BROKER_BROWSER_IMAGE,
      "runtime-broker storage helper must use the pinned host image",
    );
    this.expect(
      broker.environment?.BPANE_RUNTIME_BROKER_WORKER_CONFIG_FILE ===
        "/runtime-config/workers.json",
      "runtime-broker must use the fixed worker policy file",
    );
    this.expect(
      broker.environment?.BPANE_RUNTIME_BROKER_WORKER_OIDC_CLIENT_SECRET_FILE ===
        "/run/secrets/worker-oidc-client-secret",
      "runtime-broker must use a file-backed worker OIDC secret",
    );
    this.readOnlyMount(broker, "/runtime-config/extensions.json");
    this.readOnlyMount(broker, "/runtime-config/host-runtime.env");
    this.readOnlyMount(broker, "/runtime-config/workers.json");
    this.readOnlyMount(broker, "/run/secrets/worker-oidc-client-secret");
    this.readOnlyMount(broker, "/certs");
    this.expect(
      broker.depends_on?.["docker-proxy"]?.condition === "service_healthy",
      "runtime-broker must wait for the Docker proxy",
    );

    const command = Array.isArray(gateway.command)
      ? gateway.command.join(" ")
      : String(gateway.command ?? "");
    this.expect(
      command.includes("--runtime-backend broker_pool"),
      "gateway must select broker_pool in the overlay",
    );
    this.sameValues(
      Object.keys(gateway.networks ?? {}),
      ["bpane-internal", "runtime-broker-api"],
      "gateway broker topology networks are invalid",
    );
    this.expect(
      gateway.environment?.DOCKER_HOST == null,
      "gateway broker topology must not configure DOCKER_HOST",
    );
    this.expect(
      !Object.entries(gateway.environment ?? {}).some(
        ([key, value]) =>
          key !== "DOCKER_HOST" &&
          typeof value === "string" &&
          /(?:docker-proxy|docker\.sock|tcp:\/\/.*:237[56])/i.test(value),
      ),
      "gateway broker topology must not configure another Docker endpoint",
    );
    this.expect(
      !Object.keys(gateway.networks ?? {}).includes("docker-control"),
      "gateway broker topology must not join docker-control",
    );
    this.expect(
      !gateway.depends_on?.["docker-proxy"],
      "gateway broker topology must not depend on docker-proxy",
    );
    this.expect(
      !(gateway.volumes ?? []).some((volume) =>
        String(volume.source ?? "").includes("docker.sock"),
      ),
      "gateway broker topology must not mount the Docker socket",
    );
    this.expect(
      gateway.environment?.BPANE_GATEWAY_RUNTIME_BROKER_URL ===
        "http://runtime-broker:8940",
      "gateway must use the private runtime-broker endpoint",
    );
    this.expect(
      gateway.environment?.BPANE_GATEWAY_RUNTIME_BROKER_TOKEN_URL ===
        "http://keycloak:8080/realms/browserpane-dev/protocol/openid-connect/token",
      "gateway must use the private Keycloak token endpoint",
    );
    this.expect(
      gateway.environment?.BPANE_GATEWAY_RUNTIME_BROKER_CLIENT_ID ===
        "bpane-runtime-broker-gateway",
      "gateway must use the dedicated runtime-broker service identity",
    );
    this.readOnlyMount(gateway, "/run/secrets/runtime-broker-client-secret");
    this.expect(
      gateway.depends_on?.["runtime-broker"]?.condition === "service_healthy",
      "gateway must wait for a healthy runtime-broker",
    );
  }

  readOnlyMount(service, target) {
    const mount = (service.volumes ?? []).find((volume) => volume.target === target);
    this.expect(mount?.read_only === true, `${target} must be mounted read-only`);
  }

  isImmutableImage(value) {
    if (typeof value !== "string") return false;
    const digest = value.startsWith("sha256:")
      ? value.slice("sha256:".length)
      : value.split("@sha256:")[1];
    return /^[a-fA-F0-9]{64}$/.test(digest ?? "");
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
    if (!condition) throw new Error(message);
  }
}
