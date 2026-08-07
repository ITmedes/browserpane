import Enforcer from 'openapi-enforcer';

import { ContractExampleValidator } from './contract-example-validator.mjs';
import { ContractPaths } from './contract-paths.mjs';
import { ExampleCatalog } from './example-catalog.mjs';

try {
  const paths = ContractPaths.fromModule(import.meta.url);
  const openapi = await Enforcer(paths.contract, { hideWarnings: true });
  const examples = ExampleCatalog.load(paths.examples);
  new ContractExampleValidator(openapi).validate(examples);
  console.log(`OpenAPI examples passed (${examples.length} request/response cases).`);
} catch (error) {
  console.error(`OpenAPI examples failed: ${error.message}`);
  process.exitCode = 1;
}
