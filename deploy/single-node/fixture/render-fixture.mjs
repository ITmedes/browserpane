#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

class SingleNodeFixtureRenderer {
  render(rootDirectory) {
    const generated = path.join(rootDirectory, "deploy/single-node/generated");
    const realm = JSON.parse(fs.readFileSync(
      path.join(rootDirectory, "deploy/keycloak/browserpane-dev-realm.json"),
      "utf8",
    ));
    const browserClient = realm.clients.find((client) => client.clientId === "bpane-web");
    if (!browserClient) throw new Error("fixture realm is missing bpane-web");
    browserClient.redirectUris = ["http://localhost:18080/*"];
    browserClient.webOrigins = ["http://localhost:18080"];
    fs.writeFileSync(path.join(generated, "keycloak-realm.json"), `${JSON.stringify(realm, null, 2)}\n`, {
      mode: 0o644,
    });

    const fingerprint = fs.readFileSync(path.join(generated, "certs/cert-fingerprint.txt"), "utf8").trim();
    if (!fingerprint) throw new Error("fixture certificate fingerprint is empty");
    const workersFile = path.join(generated, "workers.json");
    const workers = JSON.parse(fs.readFileSync(workersFile, "utf8"));
    workers.recording.cert_spki = fingerprint;
    fs.writeFileSync(workersFile, `${JSON.stringify(workers, null, 2)}\n`, { mode: 0o644 });
    fs.chmodSync(workersFile, 0o644);
  }
}

try {
  new SingleNodeFixtureRenderer().render(path.resolve(import.meta.dirname, "../../.."));
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
