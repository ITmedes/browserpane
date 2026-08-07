import path from 'node:path';
import { fileURLToPath } from 'node:url';

export class ContractPaths {
  static fromModule(metaUrl) {
    const packageDirectory = path.resolve(path.dirname(fileURLToPath(metaUrl)), '..');
    const rootDirectory = path.resolve(packageDirectory, '../..');
    return new ContractPaths(rootDirectory, packageDirectory);
  }

  constructor(rootDirectory, packageDirectory) {
    this.rootDirectory = rootDirectory;
    this.packageDirectory = packageDirectory;
  }

  get contract() {
    return path.join(this.rootDirectory, 'openapi/bpane-control-v1.yaml');
  }

  get classifications() {
    return path.join(this.rootDirectory, 'openapi/bpane-control-v1.classifications.json');
  }

  get inventory() {
    return path.join(this.rootDirectory, 'openapi/bpane-control-v1.operations.json');
  }

  get examples() {
    return path.join(this.rootDirectory, 'openapi/bpane-control-v1.examples.json');
  }
}
