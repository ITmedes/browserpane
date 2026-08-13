const PROXY_IMAGE_PREFIX = "ghcr.io/tecnativa/docker-socket-proxy@sha256:";

const REQUIRED_PROXY_ENV = Object.freeze({
  ALLOW_PAUSE: "0",
  ALLOW_RESTARTS: "0",
  ALLOW_START: "1",
  ALLOW_STOP: "1",
  ALLOW_UNPAUSE: "0",
  AUTH: "0",
  BUILD: "0",
  COMMIT: "0",
  CONFIGS: "0",
  CONTAINERS: "1",
  DISTRIBUTION: "0",
  EVENTS: "0",
  EXEC: "0",
  GRPC: "0",
  IMAGES: "0",
  INFO: "1",
  NETWORKS: "0",
  NODES: "0",
  PING: "1",
  PLUGINS: "0",
  POST: "1",
  SECRETS: "0",
  SERVICES: "0",
  SESSION: "0",
  SWARM: "0",
  SYSTEM: "0",
  TASKS: "0",
  VERSION: "1",
  VOLUMES: "1",
});

export class DockerRuntimeBoundaryContract {
  validate(config) {
    const services = config?.services ?? {};
    const gateway = services.gateway;
    const proxy = services["docker-proxy"];

    this.expect(gateway, "compose service gateway is missing");
    this.expect(proxy, "compose service docker-proxy is missing");
    this.expect(
      gateway.environment?.DOCKER_HOST === "tcp://docker-proxy:2375",
      "gateway must use docker-proxy through DOCKER_HOST",
    );
    this.expect(
      gateway.depends_on?.["docker-proxy"]?.condition === "service_healthy",
      "gateway must wait for a healthy docker-proxy",
    );
    this.validateSocketMounts(services);
    this.validateProxy(proxy);
    this.validateControlNetwork(config, services, gateway, proxy);
  }

  validateSocketMounts(services) {
    const socketMounts = Object.entries(services).flatMap(([serviceName, service]) =>
      (service?.volumes ?? [])
        .filter(
          (volume) =>
            volume.source === "/var/run/docker.sock" ||
            volume.target === "/var/run/docker.sock",
        )
        .map((volume) => ({ serviceName, volume })),
    );
    this.expect(socketMounts.length === 1, "exactly one Docker socket mount is required");
    this.expect(
      socketMounts[0].serviceName === "docker-proxy",
      "only docker-proxy may mount the Docker socket",
    );
    this.expect(
      socketMounts[0].volume.read_only === true,
      "Docker socket mount must be read-only",
    );
  }

  validateProxy(proxy) {
    this.expect(
      typeof proxy.image === "string" &&
        proxy.image.startsWith(PROXY_IMAGE_PREFIX) &&
        /^[a-f0-9]{64}$/.test(proxy.image.slice(PROXY_IMAGE_PREFIX.length)),
      "docker-proxy image must use an immutable GHCR sha256 digest",
    );
    this.expect(!(proxy.ports?.length > 0), "docker-proxy must not publish host ports");
    this.expect(
      (proxy.cap_drop ?? []).includes("ALL"),
      "docker-proxy must drop all Linux capabilities",
    );
    this.expect(
      (proxy.security_opt ?? []).includes("no-new-privileges:true"),
      "docker-proxy must enable no-new-privileges",
    );
    for (const [key, expected] of Object.entries(REQUIRED_PROXY_ENV)) {
      this.expect(
        proxy.environment?.[key] === expected,
        `docker-proxy environment ${key} must be ${expected}`,
      );
    }
  }

  validateControlNetwork(config, services, gateway, proxy) {
    this.expect(
      config.networks?.["docker-control"]?.internal === true,
      "docker-control network must be internal",
    );
    this.expect(
      this.sameValues(this.networks(proxy), ["docker-control"]),
      "docker-proxy must only join docker-control",
    );
    this.expect(
      this.networks(gateway).includes("docker-control"),
      "gateway must join docker-control",
    );
    const members = Object.entries(services)
      .filter(([, service]) => this.networks(service).includes("docker-control"))
      .map(([serviceName]) => serviceName);
    this.expect(
      this.sameValues(members, ["docker-proxy", "gateway"]),
      "docker-control must contain only docker-proxy and gateway",
    );
  }

  networks(service) {
    return Object.keys(service?.networks ?? {});
  }

  sameValues(left, right) {
    return JSON.stringify([...left].sort()) === JSON.stringify([...right].sort());
  }

  expect(condition, message) {
    if (!condition) {
      throw new Error(message);
    }
  }
}
