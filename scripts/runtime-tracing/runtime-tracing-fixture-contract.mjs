const COLLECTOR_IMAGE =
  "otel/opentelemetry-collector-contrib@sha256:e0f565815b2b8e78eb9fdbb80f6190c921ab323aaa8ccceab3255c6d4225f4af";
const OTLP_ENDPOINT = "http://otel-collector:4317";

export class RuntimeTracingFixtureContract {
  validate(config) {
    const services = config.services ?? {};
    const collector = services["otel-collector"];
    this.expect(collector, "runtime tracing fixture is missing otel-collector");
    this.expect(collector.image === COLLECTOR_IMAGE, "collector image is not digest pinned");
    this.expect(!(collector.ports?.length > 0), "collector must not publish host ports");
    this.same(Object.keys(collector.networks ?? {}), ["broker-api"],
      "collector networks are invalid");
    this.expect(collector.read_only === true, "collector root filesystem must be read-only");
    this.same(collector.cap_drop, ["ALL"], "collector capabilities must be dropped");
    this.expect(
      collector.security_opt?.includes("no-new-privileges:true"),
      "collector must disable privilege escalation",
    );
    const configMount = collector.volumes?.find(
      (mount) => mount.target === "/etc/otelcol-contrib/config.yaml",
    );
    this.expect(
      configMount?.type === "bind" && configMount.read_only === true,
      "collector configuration must be a read-only bind mount",
    );
    const traceMount = collector.volumes?.find(
      (mount) => mount.target === "/var/lib/otel",
    );
    this.expect(
      traceMount?.type === "bind" && traceMount.read_only !== true,
      "collector trace output must use the generated writable bind mount",
    );
    this.expect(config.networks?.["broker-api"]?.internal === true,
      "collector network must remain internal");

    for (const serviceName of ["gateway", "runtime-broker"]) {
      const service = services[serviceName];
      const environment = service?.environment ?? {};
      this.expect(environment.OTEL_TRACES_EXPORTER === "otlp",
        `${serviceName} must explicitly enable OTLP traces`);
      this.expect(environment.OTEL_EXPORTER_OTLP_ENDPOINT === OTLP_ENDPOINT,
        `${serviceName} must use the private collector endpoint`);
      this.expect(environment.OTEL_EXPORTER_OTLP_PROTOCOL === "grpc",
        `${serviceName} must use OTLP gRPC`);
      this.expect(environment.OTEL_TRACES_SAMPLER === "always_on",
        `${serviceName} fixture sampling must be deterministic`);
      this.expect(service.depends_on?.["otel-collector"]?.condition === "service_started",
        `${serviceName} must start after the collector`);
      this.expect(Object.hasOwn(service.networks ?? {}, "broker-api"),
        `${serviceName} must share only the existing private broker path with the collector`);
    }

    for (const [serviceName, service] of Object.entries(services)) {
      if (["gateway", "runtime-broker"].includes(serviceName)) continue;
      this.expect(
        !Object.keys(service.environment ?? {}).some((name) => name.startsWith("OTEL_")),
        `${serviceName} must not receive trace exporter configuration`,
      );
    }
  }

  expect(condition, message) {
    if (!condition) throw new Error(message);
  }

  same(actual, expected, message) {
    const sortedActual = [...actual].sort();
    const sortedExpected = [...expected].sort();
    this.expect(JSON.stringify(sortedActual) === JSON.stringify(sortedExpected), message);
  }
}

export const runtimeTracingCollectorImage = COLLECTOR_IMAGE;
