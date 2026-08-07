import openapiDiff from 'openapi-diff';

import { OpenApiContract } from './openapi-contract.mjs';

export class SemanticCompatibilityChecker {
  constructor(engine = openapiDiff) {
    this.engine = engine;
  }

  async compare(baseContent, revisionContent, labels = {}) {
    OpenApiContract.parse(baseContent);
    OpenApiContract.parse(revisionContent);
    const result = await this.engine.diffSpecs({
      sourceSpec: {
        content: baseContent,
        location: labels.base ?? 'base.yaml',
        format: 'openapi3'
      },
      destinationSpec: {
        content: revisionContent,
        location: labels.revision ?? 'revision.yaml',
        format: 'openapi3'
      }
    });
    if (result.breakingDifferencesFound) {
      throw new Error(this.#formatBreaking(result.breakingDifferences));
    }
    if ((result.unclassifiedDifferences?.length ?? 0) > 0) {
      throw new Error(this.#formatUnclassified(result.unclassifiedDifferences));
    }
    return {
      nonBreakingChanges: result.nonBreakingDifferences?.length ?? 0,
      unclassifiedChanges: 0
    };
  }

  #formatBreaking(differences) {
    const details = (differences ?? []).slice(0, 20).map((difference) => {
      const location = difference.sourceSpecEntityDetails?.[0]?.location ?? 'unknown location';
      return `- ${difference.code ?? difference.action ?? 'breaking change'} at ${location}`;
    });
    return `breaking OpenAPI changes detected:\n${details.join('\n')}`;
  }

  #formatUnclassified(differences) {
    const details = (differences ?? []).slice(0, 20).map((difference) => {
      const location = difference.destinationSpecEntityDetails?.[0]?.location
        ?? difference.sourceSpecEntityDetails?.[0]?.location
        ?? 'unknown location';
      return `- ${difference.code ?? difference.action ?? 'unclassified change'} at ${location}`;
    });
    return `unclassified OpenAPI changes require review:\n${details.join('\n')}`;
  }
}
