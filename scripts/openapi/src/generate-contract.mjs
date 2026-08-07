import { ContractLoader } from './contract-loader.mjs';
import { ContractPaths } from './contract-paths.mjs';
import { InventoryFile } from './inventory-file.mjs';

try {
  const paths = ContractPaths.fromModule(import.meta.url);
  const inventory = ContractLoader.buildInventory(paths);
  InventoryFile.write(paths.inventory, inventory);
  console.log(`Generated OpenAPI inventory (${inventory.operations.length} operations).`);
} catch (error) {
  console.error(`OpenAPI inventory generation failed: ${error.message}`);
  process.exitCode = 1;
}
