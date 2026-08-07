import { CompatibilitySurfaceCatalog } from './compatibility-surface-catalog.mjs';
import { ContractLoader } from './contract-loader.mjs';
import { ContractPaths } from './contract-paths.mjs';

try {
  const paths = ContractPaths.fromModule(import.meta.url);
  const inventory = ContractLoader.buildInventory(paths);
  const surfaces = CompatibilitySurfaceCatalog.load(paths.compatibilitySurfaces);
  CompatibilitySurfaceCatalog.validateAgainstInventory(surfaces, inventory.operations);
  console.log(`OpenAPI compatibility surfaces passed (${surfaces.length} non-v1 surfaces).`);
} catch (error) {
  console.error(`OpenAPI compatibility surfaces failed: ${error.message}`);
  process.exitCode = 1;
}
