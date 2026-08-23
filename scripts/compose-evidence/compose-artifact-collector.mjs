import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const MAX_DIAGNOSTICS_BYTES = 3 * 1024 * 1024;
const MAX_ATTACHMENT_BYTES = 5 * 1024 * 1024;
const MAX_ATTACHMENT_COUNT = 8;
const MAX_TOTAL_ATTACHMENT_BYTES = 20 * 1024 * 1024;
const FILE_PATTERN = /^[a-z0-9][a-z0-9._-]{0,80}\.(?:png|webp|zip)$/;

export class ComposeArtifactCollector {
  #rootDirectory;

  constructor(rootDirectory) {
    this.#rootDirectory = rootDirectory;
  }

  collect(laneDirectory) {
    const artifacts = [];
    const errors = [];
    this.#collectDiagnostics(artifacts, errors);
    this.#collectSafeAttachments(laneDirectory, artifacts, errors);
    return { artifacts, errors };
  }

  #collectDiagnostics(artifacts, errors) {
    const relativePath = 'test-results/ci-diagnostics/compose.log';
    const absolutePath = path.join(this.#rootDirectory, relativePath);
    if (!fs.existsSync(absolutePath)) return;
    this.#recordFile(absolutePath, relativePath, 'diagnostics', MAX_DIAGNOSTICS_BYTES,
      artifacts, errors);
  }

  #collectSafeAttachments(laneDirectory, artifacts, errors) {
    const sourceDirectory = path.join(this.#rootDirectory, 'test-results/compose-safe-attachments');
    const manifestPath = path.join(sourceDirectory, 'manifest.json');
    if (!fs.existsSync(manifestPath)) return;
    let manifest;
    try {
      manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
    } catch {
      errors.push('attachment-manifest-invalid');
      return;
    }
    if (manifest.schema_version !== 1 || !Array.isArray(manifest.artifacts)
      || manifest.artifacts.length > MAX_ATTACHMENT_COUNT) {
      errors.push('attachment-manifest-invalid');
      return;
    }
    const destination = path.join(laneDirectory, 'attachments');
    let retainedBytes = 0;
    for (const item of manifest.artifacts) {
      if (!this.#validAttachment(item)) {
        errors.push('attachment-entry-invalid');
        continue;
      }
      const sourcePath = path.join(sourceDirectory, item.file);
      const destinationPath = path.join(destination, item.file);
      try {
        if (retainedBytes + fs.lstatSync(sourcePath).size > MAX_TOTAL_ATTACHMENT_BYTES) {
          errors.push('attachment-total-size-rejected');
          continue;
        }
      } catch {
        errors.push(`${item.kind}-artifact-unavailable`);
        continue;
      }
      const before = artifacts.length;
      this.#recordFile(sourcePath, `attachments/${item.file}`, item.kind,
        MAX_ATTACHMENT_BYTES, artifacts, errors);
      if (artifacts.length > before) {
        retainedBytes += artifacts.at(-1).bytes;
        fs.mkdirSync(destination, { recursive: true, mode: 0o700 });
        fs.copyFileSync(sourcePath, destinationPath);
      }
    }
  }

  #validAttachment(item) {
    if (!item || typeof item !== 'object' || !FILE_PATTERN.test(item.file ?? '')) return false;
    if (item.content !== 'synthetic-fixture') return false;
    if (item.kind === 'trace') return item.file.endsWith('.zip');
    return item.kind === 'screenshot' && /\.(?:png|webp)$/.test(item.file);
  }

  #recordFile(absolutePath, relativePath, kind, maximumBytes, artifacts, errors) {
    let stat;
    try {
      stat = fs.lstatSync(absolutePath);
    } catch {
      errors.push(`${kind}-artifact-unavailable`);
      return;
    }
    if (!stat.isFile() || stat.isSymbolicLink() || stat.size > maximumBytes) {
      errors.push(`${kind}-artifact-rejected`);
      return;
    }
    const content = fs.readFileSync(absolutePath);
    artifacts.push({
      kind,
      path: relativePath,
      bytes: stat.size,
      sha256: crypto.createHash('sha256').update(content).digest('hex'),
    });
  }
}
