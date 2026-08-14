import { RuntimeBrokerServiceSecurityContract } from "../runtime-broker/runtime-broker-service-security-contract.mjs";

const SERVICES = ["docker-proxy", "gateway", "runtime-broker", "web"];
const DURABLE_VOLUMES = [
  "file-workspaces",
  "recording-artifacts",
  "recording-staging",
  "runtime-sockets",
];

export class SingleNodeComposeContract {
  validate(input) {
    const config = input.composeConfig;
    const services = config.services ?? {};
    this.same(Object.keys(services), SERVICES, "single-node service set is invalid");
    this.same(Object.keys(config.volumes ?? {}), DURABLE_VOLUMES,
      "single-node durable volume set is invalid");
    for (const [name, service] of Object.entries(services)) {
      this.immutable(service.image, `${name} image must be immutable`);
      this.expect(!service.build, `${name} must not build from a checkout`);
      this.expect(service.restart === "unless-stopped", `${name} must restart unless stopped`);
      this.expect(service.read_only === true, `${name} root filesystem must be read-only`);
      this.expect((service.cap_drop ?? []).includes("ALL"), `${name} must drop all capabilities`);
      this.expect((service.security_opt ?? []).includes("no-new-privileges:true"),
        `${name} must enable no-new-privileges`);
      this.expect(service.privileged !== true, `${name} must not be privileged`);
    }

    const web = services.web;
    const gateway = services.gateway;
    const broker = services["runtime-broker"];
    const proxy = services["docker-proxy"];
    new RuntimeBrokerServiceSecurityContract().validate(broker);
    this.same(Object.keys(web.networks ?? {}), ["runtime"], "web networks are invalid");
    this.same(Object.keys(gateway.networks ?? {}), ["broker-api", "runtime"],
      "gateway networks are invalid");
    this.same(gateway.group_add ?? [], ["1001"],
      "gateway must join only the browser runtime socket group");
    this.same(Object.keys(broker.networks ?? {}), ["broker-api", "broker-auth", "docker-control"],
      "runtime-broker networks are invalid");
    this.same(Object.keys(proxy.networks ?? {}), ["docker-control"],
      "docker-proxy networks are invalid");
    this.expect(config.networks?.["broker-api"]?.internal === true,
      "broker-api must be internal");
    this.expect(config.networks?.["docker-control"]?.internal === true,
      "docker-control must be internal");
    this.same(this.networkMembers(services, "docker-control"), ["docker-proxy", "runtime-broker"],
      "docker-control membership is invalid");

    this.webPort(web);
    this.same(web.healthcheck?.test ?? [], ["CMD", "wget", "-qO-", "http://127.0.0.1:8080/healthz"],
      "web healthcheck must use IPv4 loopback");
    this.gatewayPorts(gateway);
    this.expect(!(broker.ports?.length > 0), "runtime-broker must not publish ports");
    this.expect(!(proxy.ports?.length > 0), "docker-proxy must not publish ports");
    const command = gateway.command ?? [];
    this.expect(this.argumentValue(command, "--runtime-backend") === "broker_pool",
      "gateway must use broker_pool");
    this.expect(
      this.argumentValue(command, "--docker-runtime-container-name-prefix") ===
        broker.environment?.BPANE_RUNTIME_BROKER_CONTAINER_PREFIX,
      "gateway and runtime-broker container prefixes must match",
    );
    for (const flag of [
      "--database-url-file",
      "--credential-vault-token-file",
      "--runtime-broker-client-secret-file",
    ]) this.expect(this.argumentValue(command, flag)?.startsWith("/run/secrets/"),
      `${flag} must use a secret file`);
    for (const flag of ["--database-url", "--credential-vault-token", "--mcp-bridge-control-token"])
      this.expect(!command.includes(flag), `${flag} inline configuration is forbidden`);
    this.expect(!gateway.environment?.DOCKER_HOST, "gateway must not configure DOCKER_HOST");
    this.expect(!gateway.depends_on?.["docker-proxy"], "gateway must not depend on docker-proxy");
    this.expect(!Object.keys(gateway.networks ?? {}).includes("docker-control"),
      "gateway must not join docker-control");
    this.expect(!(gateway.volumes ?? []).some((mount) => String(mount.source).includes("docker.sock")),
      "gateway must not mount the Docker socket");
    for (const target of [
      "/run/secrets/database-url",
      "/run/secrets/vault-token",
      "/run/secrets/broker-gateway-client-secret",
      "/run/tls/cert.pem",
      "/run/tls/cert.key",
      "/runtime-config/browser.env",
    ]) this.mount(gateway, target, "bind", true);
    this.mount(gateway, "/var/lib/browserpane/recording-artifacts", "volume", false);
    this.mount(gateway, "/var/lib/browserpane/file-workspaces", "volume", false);
    for (const target of [
      "/runtime-config/extensions.json",
      "/runtime-config/browser.env",
      "/runtime-config/workers.json",
      "/run/secrets/worker-oidc-client-secret",
    ]) this.mount(broker, target, "bind", true);
    for (const name of [
      "BPANE_RUNTIME_BROKER_BROWSER_IMAGE",
      "BPANE_RUNTIME_BROKER_WORKFLOW_IMAGE",
      "BPANE_RUNTIME_BROKER_RECORDING_IMAGE",
      "BPANE_RUNTIME_BROKER_STORAGE_HELPER_IMAGE",
    ]) this.immutable(broker.environment?.[name], `${name} must be immutable`);
    this.expect(
      broker.environment?.BPANE_RUNTIME_BROKER_STORAGE_HELPER_IMAGE ===
        broker.environment?.BPANE_RUNTIME_BROKER_BROWSER_IMAGE,
      "runtime-broker storage helper must use the browser image",
    );
    this.expect(broker.depends_on?.["docker-proxy"]?.condition === "service_healthy",
      "runtime-broker must wait for docker-proxy");
    this.expect(gateway.depends_on?.["runtime-broker"]?.condition === "service_healthy",
      "gateway must wait for runtime-broker");
    this.proxyPolicy(proxy);
    this.webSurface(input);
  }

  webPort(web) {
    const ports = web.ports ?? [];
    this.expect(ports.length === 1 && ports[0].target === 8080, "web must publish only port 8080");
    this.expect(["127.0.0.1", "::1"].includes(ports[0].host_ip),
      "web management listener must bind loopback");
  }

  gatewayPorts(gateway) {
    const ports = gateway.ports ?? [];
    this.expect(ports.length === 2, "gateway must publish only WebTransport TCP and UDP");
    this.same(ports.map((port) => `${port.target}/${port.protocol}`), ["4433/tcp", "4433/udp"],
      "gateway WebTransport listeners are invalid");
  }

  proxyPolicy(proxy) {
    for (const name of ["AUTH", "BUILD", "COMMIT", "EXEC", "IMAGES", "NETWORKS", "SECRETS",
      "SERVICES", "SWARM", "SYSTEM", "TASKS"]) {
      this.expect(String(proxy.environment?.[name]) === "0", `docker-proxy ${name} must be denied`);
    }
    this.expect(String(proxy.environment?.CONTAINERS) === "1", "docker-proxy containers must be enabled");
    this.expect(String(proxy.environment?.VOLUMES) === "1", "docker-proxy volumes must be enabled");
  }

  webSurface(input) {
    for (const marker of ["dev/web-fixtures", "web-cert-metadata", "cert-fingerprint", "cert-hash"]) {
      this.expect(!input.webDockerfile.includes(marker), `production web image contains ${marker}`);
    }
    for (const marker of ["/cert-hash", "/cert-fingerprint", "/test/workflow-events"]) {
      this.expect(!input.nginxConfig.includes(marker), `production nginx exposes ${marker}`);
    }
    this.expect(!input.authTemplate.includes("exampleUser"),
      "production auth config must not contain example credentials");
    for (const marker of ["browserpane-dev", "demo-demo", "start-dev", "../dev/certs", "/workspace:ro"])
      this.expect(!input.composeSource.includes(marker), `single-node compose contains ${marker}`);
  }

  mount(service, target, type, readOnly) {
    const mount = (service.volumes ?? []).find((entry) => entry.target === target);
    this.expect(mount?.type === type, `${target} must use a ${type} mount`);
    this.expect(Boolean(mount?.read_only) === readOnly, `${target} read-only policy is invalid`);
  }

  argumentValue(command, flag) {
    const index = command.indexOf(flag);
    return index < 0 ? undefined : command[index + 1];
  }

  immutable(value, message) {
    const digest = value?.startsWith("sha256:") ? value.slice(7) : value?.split("@sha256:")[1];
    this.expect(/^[a-fA-F0-9]{64}$/u.test(digest ?? ""), message);
  }

  networkMembers(services, network) {
    return Object.entries(services).filter(([, value]) => Object.hasOwn(value.networks ?? {}, network))
      .map(([name]) => name);
  }

  same(actual, expected, message) {
    this.expect(JSON.stringify([...actual].sort()) === JSON.stringify([...expected].sort()), message);
  }

  expect(condition, message) {
    if (!condition) throw new Error(message);
  }
}
