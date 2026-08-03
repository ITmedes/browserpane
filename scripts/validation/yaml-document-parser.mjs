import { spawnSync } from 'node:child_process';

const RUBY_PARSER = String.raw`
path = ARGV.fetch(0)
value = YAML.safe_load(
  File.read(path),
  permitted_classes: [],
  permitted_symbols: [],
  aliases: true
)
STDOUT.write(JSON.generate(value))
`;

export class YamlDocumentParser {
  parse(path) {
    const result = spawnSync('ruby', ['-ryaml', '-rjson', '-e', RUBY_PARSER, path], {
      encoding: 'utf8',
      maxBuffer: 10 * 1024 * 1024
    });
    if (result.error) {
      throw new Error(`unable to run Ruby YAML parser: ${result.error.message}`);
    }
    if (result.status !== 0) {
      throw new Error(String(result.stderr).trim() || `YAML parser exited ${result.status}`);
    }
    return JSON.parse(result.stdout);
  }
}
