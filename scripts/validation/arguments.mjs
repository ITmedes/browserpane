export class ValidationArguments {
  static parse(args) {
    const result = {
      profile: null,
      requestedStages: [],
      dryRun: false,
      list: false,
      help: false
    };

    for (let index = 0; index < args.length; index += 1) {
      const argument = args[index];
      if (argument === '--profile') {
        result.profile = this.#value(args, ++index, '--profile');
      } else if (argument === '--stage') {
        result.requestedStages.push(this.#value(args, ++index, '--stage'));
      } else if (argument === '--dry-run') {
        result.dryRun = true;
      } else if (argument === '--list') {
        result.list = true;
      } else if (argument === '--help' || argument === '-h') {
        result.help = true;
      } else {
        throw new Error(`unknown argument: ${argument}`);
      }
    }

    if (result.profile && !['fast', 'compose', 'full'].includes(result.profile)) {
      throw new Error(`unknown validation profile: ${result.profile}`);
    }
    if (new Set(result.requestedStages).size !== result.requestedStages.length) {
      throw new Error('validation stages must not be repeated');
    }
    return Object.freeze({
      ...result,
      requestedStages: Object.freeze([...result.requestedStages]),
      selectionProfile: result.profile ?? (result.requestedStages.length > 0 ? 'all' : 'fast')
    });
  }

  static #value(args, index, option) {
    const value = args[index];
    if (!value || value.startsWith('--')) {
      throw new Error(`${option} requires a value`);
    }
    return value;
  }
}
