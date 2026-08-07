import fs from 'node:fs';
import path from 'node:path';

import { CompatibilityArguments } from './compatibility-arguments.mjs';
import { ContractPaths } from './contract-paths.mjs';
import { GitContractBaseline } from './git-contract-baseline.mjs';
import { SemanticCompatibilityChecker } from './semantic-compatibility-checker.mjs';

function printHelp() {
  console.log(`Usage: npm run compatibility -- [--base-ref <git-revision>]

Compares the frozen control API at the selected git revision with the working
tree contract. The default base revision is BPANE_OPENAPI_BASE_REF or
origin/main.`);
}

try {
  const options = CompatibilityArguments.parse(process.argv.slice(2));
  if (options.help) {
    printHelp();
  } else {
    const paths = ContractPaths.fromModule(import.meta.url);
    const relativeContract = path.relative(paths.rootDirectory, paths.contract);
    const baseline = new GitContractBaseline(paths.rootDirectory)
      .load(options.baseRef, relativeContract);
    const revision = fs.readFileSync(paths.contract, 'utf8');
    const result = await new SemanticCompatibilityChecker().compare(baseline, revision, {
      base: `${options.baseRef}:${relativeContract}`,
      revision: relativeContract
    });
    console.log(
      `OpenAPI compatibility passed (${result.nonBreakingChanges} additive, `
      + `${result.unclassifiedChanges} unclassified changes).`
    );
  }
} catch (error) {
  console.error(`OpenAPI compatibility failed: ${error.message}`);
  process.exitCode = 1;
}
