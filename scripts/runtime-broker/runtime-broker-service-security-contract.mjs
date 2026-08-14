const EXPECTED_TMPFS = "/tmp:size=16m,mode=1777";

export class RuntimeBrokerServiceSecurityContract {
  validate(broker) {
    this.expect(broker, "compose service runtime-broker is missing");
    this.expect(!(broker.ports?.length > 0), "runtime-broker must not publish host ports");
    this.expect(
      broker.read_only === true,
      "runtime-broker root filesystem must be read-only",
    );
    this.expect(
      (broker.cap_drop ?? []).includes("ALL"),
      "runtime-broker must drop all Linux capabilities",
    );
    this.expect(
      (broker.security_opt ?? []).includes("no-new-privileges:true"),
      "runtime-broker must enable no-new-privileges",
    );
    this.expect(
      broker.privileged !== true,
      "runtime-broker must not run as privileged",
    );
    this.expect(
      broker.pid !== "host" && broker.ipc !== "host",
      "runtime-broker must not join host PID or IPC namespaces",
    );
    this.expect(
      !(broker.devices?.length > 0),
      "runtime-broker must not mount host devices",
    );
    this.expect(
      (broker.tmpfs ?? []).includes(EXPECTED_TMPFS),
      "runtime-broker must use the bounded writable /tmp filesystem",
    );
    this.expect(
      !(broker.volumes ?? []).some((volume) =>
        String(volume.source ?? "").includes("docker.sock")),
      "runtime-broker must not mount the Docker socket",
    );
    this.expect(
      broker.healthcheck?.test?.join(" ").includes("/readyz"),
      "runtime-broker healthcheck must use /readyz",
    );
  }

  expect(condition, message) {
    if (!condition) throw new Error(message);
  }
}
