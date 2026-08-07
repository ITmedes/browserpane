export class CompatibilityArguments {
  static parse(rawArguments, environment = process.env) {
    let baseRef = environment.BPANE_OPENAPI_BASE_REF ?? 'origin/main';
    let baseRefSeen = false;
    let help = false;
    for (let index = 0; index < rawArguments.length; index += 1) {
      const argument = rawArguments[index];
      if (argument === '--help') {
        help = true;
      } else if (argument === '--base-ref') {
        if (baseRefSeen) throw new Error('--base-ref may be provided only once');
        const value = rawArguments[index + 1];
        if (typeof value !== 'string' || value.length === 0 || value.startsWith('--')) {
          throw new Error('--base-ref requires a non-empty git revision');
        }
        baseRef = value;
        baseRefSeen = true;
        index += 1;
      } else {
        throw new Error(`unknown argument: ${argument}`);
      }
    }
    if (typeof baseRef !== 'string' || baseRef.trim().length === 0) {
      throw new Error('--base-ref requires a non-empty git revision');
    }
    return { baseRef: baseRef.trim(), help };
  }
}
