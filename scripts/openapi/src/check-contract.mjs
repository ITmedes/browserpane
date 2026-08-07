import { ContractLoader } from './contract-loader.mjs';
import { ContractPaths } from './contract-paths.mjs';
import { InventoryFile } from './inventory-file.mjs';

try {
  const paths = ContractPaths.fromModule(import.meta.url);
  const inventory = ContractLoader.buildInventory(paths);
  InventoryFile.check(paths.inventory, inventory);
  console.log(`OpenAPI inventory passed (${inventory.operations.length} operations).`);
} catch (error) {
  console.error(`OpenAPI inventory failed: ${error.message}`);
  process.exitCode = 1;
}
