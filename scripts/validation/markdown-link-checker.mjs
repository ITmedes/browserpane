import fs from 'node:fs';
import path from 'node:path';

export class MarkdownLinkChecker {
  #rootDirectory;

  constructor(rootDirectory) {
    this.#rootDirectory = rootDirectory;
  }

  check(relativePath) {
    const absolutePath = path.join(this.#rootDirectory, relativePath);
    const content = fs.readFileSync(absolutePath, 'utf8');
    const errors = [];
    for (const target of this.#extractTargets(content)) {
      const localPath = this.#toLocalPath(target, relativePath);
      if (localPath && !fs.existsSync(path.join(this.#rootDirectory, localPath))) {
        errors.push(`${relativePath} links to missing local path: ${target}`);
      }
    }
    return errors;
  }

  #extractTargets(content) {
    const targets = [];
    const visibleContent = content.replace(/```[\s\S]*?```/g, '');
    const markdownPattern = /!?\[[^\]]*\]\(([^)]+)\)/g;
    const htmlPattern = /\b(?:href|src)=["']([^"']+)["']/gi;
    for (const pattern of [markdownPattern, htmlPattern]) {
      for (const match of visibleContent.matchAll(pattern)) targets.push(match[1].trim());
    }
    return targets;
  }

  #toLocalPath(rawTarget, sourcePath) {
    const target = this.#withoutTitle(rawTarget);
    if (!target || target.startsWith('#') || /^[a-z][a-z0-9+.-]*:/i.test(target)) return null;
    const withoutFragment = target.split('#', 1)[0].split('?', 1)[0];
    if (!withoutFragment) return null;
    let decoded;
    try {
      decoded = decodeURIComponent(withoutFragment.replace(/^<|>$/g, ''));
    } catch {
      return '__invalid_percent_encoding__';
    }
    const relative = decoded.startsWith('/')
      ? decoded.slice(1)
      : path.join(path.dirname(sourcePath), decoded);
    return path.normalize(relative);
  }

  #withoutTitle(target) {
    if (target.startsWith('<')) {
      const closing = target.indexOf('>');
      return closing >= 0 ? target.slice(0, closing + 1) : target;
    }
    return target.split(/\s+["']/u, 1)[0];
  }
}
