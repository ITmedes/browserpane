import { promises as fs } from "node:fs";
import path from "node:path";
import { unzipSync } from "fflate";

type MaterializeSourceSnapshotRequest = {
  archiveBytes: Uint8Array;
  destinationRoot: string;
};

export type MaterializedSourceSnapshot = {
  rootPath: string;
};

export async function materializeSourceSnapshot(
  request: MaterializeSourceSnapshotRequest,
): Promise<MaterializedSourceSnapshot> {
  await fs.rm(request.destinationRoot, { recursive: true, force: true }).catch(() => {});
  await fs.mkdir(request.destinationRoot, { recursive: true });

  const entries = unzipSync(request.archiveBytes);
  for (const [entryName, entryBytes] of Object.entries(entries)) {
    if (!entryName || entryName.endsWith("/")) {
      continue;
    }
    const normalizedPath = normalizeArchiveEntry(entryName);
    const outputPath = path.join(request.destinationRoot, normalizedPath);
    await fs.mkdir(path.dirname(outputPath), { recursive: true });
    await fs.writeFile(outputPath, Buffer.from(entryBytes));
  }

  return { rootPath: request.destinationRoot };
}

function normalizeArchiveEntry(entryName: string): string {
  const normalized = path.posix.normalize(entryName.trim());
  if (
    !normalized ||
    normalized.startsWith("/") ||
    normalized === "." ||
    normalized === ".." ||
    normalized.split("/").some((segment) => segment === ".." || !segment)
  ) {
    throw new Error(`invalid source snapshot archive entry: ${entryName}`);
  }
  return normalized;
}
