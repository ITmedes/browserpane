import { copyFile, mkdir } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(scriptDir, '..');
const sourcePath = resolve(projectRoot, 'js', 'audio', 'audio-worklet.js');
const targetPath = resolve(projectRoot, 'dist', 'audio-worklet.js');

await mkdir(dirname(targetPath), { recursive: true });
await copyFile(sourcePath, targetPath);
