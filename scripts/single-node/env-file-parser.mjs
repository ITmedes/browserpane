import fs from "node:fs";

export class EnvironmentFileParser {
  parse(filename) {
    const environment = {};
    const lines = fs.readFileSync(filename, "utf8").split(/\r?\n/u);
    for (let index = 0; index < lines.length; index += 1) {
      const line = lines[index].trim();
      if (!line || line.startsWith("#")) continue;
      const separator = line.indexOf("=");
      if (separator <= 0) throw new Error(`invalid environment entry on line ${index + 1}`);
      const name = line.slice(0, separator).trim();
      const value = this.unquote(line.slice(separator + 1).trim(), index + 1);
      if (!/^[A-Z][A-Z0-9_]*$/u.test(name)) {
        throw new Error(`invalid environment name on line ${index + 1}`);
      }
      if (Object.hasOwn(environment, name)) throw new Error(`duplicate environment entry: ${name}`);
      environment[name] = value;
    }
    return environment;
  }

  unquote(value, lineNumber) {
    if (!value.startsWith('"') && !value.startsWith("'")) return value;
    const quote = value[0];
    if (value.length < 2 || value.at(-1) !== quote) {
      throw new Error(`unterminated environment value on line ${lineNumber}`);
    }
    return value.slice(1, -1);
  }
}
