import path from 'node:path';

import { GitHubWorkflowPolicyChecker } from './github-workflow-policy-checker.mjs';
import { MarkdownLinkChecker } from './markdown-link-checker.mjs';
import { YamlDocumentParser } from './yaml-document-parser.mjs';

export class RepositoryDocumentChecker {
  #rootDirectory;
  #markdownChecker;
  #workflowChecker;
  #yamlParser;

  constructor(rootDirectory, dependencies = {}) {
    this.#rootDirectory = rootDirectory;
    this.#markdownChecker = dependencies.markdownChecker
      ?? new MarkdownLinkChecker(rootDirectory);
    this.#workflowChecker = dependencies.workflowChecker
      ?? new GitHubWorkflowPolicyChecker();
    this.#yamlParser = dependencies.yamlParser ?? new YamlDocumentParser();
  }

  check({ markdownFiles, yamlFiles, workflowFiles }) {
    const errors = [];
    for (const relativePath of markdownFiles) {
      try {
        errors.push(...this.#markdownChecker.check(relativePath));
      } catch (error) {
        errors.push(`${relativePath} could not be checked: ${error.message}`);
      }
    }
    const parsedYaml = new Map();
    for (const relativePath of yamlFiles) {
      try {
        parsedYaml.set(relativePath, this.#yamlParser.parse(
          path.join(this.#rootDirectory, relativePath)
        ));
      } catch (error) {
        errors.push(`${relativePath} is invalid YAML: ${error.message}`);
      }
    }
    for (const relativePath of workflowFiles) {
      const workflow = parsedYaml.get(relativePath);
      if (workflow !== undefined) {
        errors.push(...this.#workflowChecker.check(relativePath, workflow));
      }
    }
    return errors;
  }
}
