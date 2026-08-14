import { lstat, readFile, unlink } from "node:fs/promises";
import { isAbsolute } from "node:path";

const MAX_SECRET_FILE_BYTES = 64 * 1024;
const MAX_SECRET_VALUE_BYTES = 16 * 1024;

export class WorkerSecretStore {
  private readonly allowedKeys: ReadonlySet<string>;

  public constructor(
    allowedKeys: readonly string[],
    private readonly environment: NodeJS.ProcessEnv = process.env,
    private readonly waitTimeoutMs = 10_000,
    private readonly pollIntervalMs = 25,
  ) {
    this.allowedKeys = new Set(allowedKeys);
  }

  public async load(): Promise<Readonly<Record<string, string>>> {
    const filePath = (this.environment.BPANE_WORKER_SECRETS_FILE ?? "").trim();
    if (!filePath) {
      return this.fromEnvironment();
    }
    this.validatePath(filePath);
    const payload = await this.readOnceAndDelete(filePath);
    return this.parse(payload);
  }

  private fromEnvironment(): Readonly<Record<string, string>> {
    const secrets: Record<string, string> = {};
    for (const key of this.allowedKeys) {
      const value = this.environment[key];
      if (value) {
        secrets[key] = value;
      }
    }
    return Object.freeze(secrets);
  }

  private validatePath(filePath: string): void {
    if (!isAbsolute(filePath) || filePath.length > 512 || /[\0\r\n]/u.test(filePath)) {
      throw new Error("worker secrets file path is invalid");
    }
  }

  private async readOnceAndDelete(filePath: string): Promise<string> {
    const deadline = Date.now() + this.waitTimeoutMs;
    for (;;) {
      try {
        const metadata = await lstat(filePath);
        if (
          !metadata.isFile()
          || metadata.isSymbolicLink()
          || metadata.size <= 0
          || metadata.size > MAX_SECRET_FILE_BYTES
          || (metadata.mode & 0o077) !== 0
        ) {
          throw new Error("invalid worker secrets file");
        }
        const payload = await readFile(filePath, "utf8");
        await unlink(filePath);
        return payload;
      } catch (error) {
        if (this.isMissing(error) && Date.now() < deadline) {
          await new Promise((resolve) => setTimeout(resolve, this.pollIntervalMs));
          continue;
        }
        throw new Error("worker secrets file is unavailable");
      }
    }
  }

  private parse(payload: string): Readonly<Record<string, string>> {
    let value: unknown;
    try {
      value = JSON.parse(payload);
    } catch {
      throw new Error("worker secrets file is invalid");
    }
    if (typeof value !== "object" || value === null || Array.isArray(value)) {
      throw new Error("worker secrets file is invalid");
    }
    const secrets: Record<string, string> = {};
    for (const [key, secret] of Object.entries(value)) {
      if (!this.allowedKeys.has(key) || !this.isValidSecret(secret)) {
        throw new Error("worker secrets file is invalid");
      }
      secrets[key] = secret;
    }
    return Object.freeze(secrets);
  }

  private isValidSecret(value: unknown): value is string {
    return typeof value === "string"
      && value.length > 0
      && Buffer.byteLength(value, "utf8") <= MAX_SECRET_VALUE_BYTES
      && !/[\0\r\n]/u.test(value);
  }

  private isMissing(error: unknown): boolean {
    return error instanceof Error && "code" in error && error.code === "ENOENT";
  }
}
