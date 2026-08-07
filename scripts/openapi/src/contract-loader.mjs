import { ClassificationPolicy } from './classification-policy.mjs';
import { ContractInventory } from './contract-inventory.mjs';
import { OpenApiContract } from './openapi-contract.mjs';

export class ContractLoader {
  static buildInventory(paths) {
    const contract = OpenApiContract.load(paths.contract);
    const classifications = ClassificationPolicy.load(paths.classifications);
    return new ContractInventory(contract, classifications).build();
  }
}
