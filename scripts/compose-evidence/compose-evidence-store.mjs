import fs from 'node:fs';
import path from 'node:path';

const LANE_PATTERN = /^[a-z0-9-]{1,48}$/;
const STAGE_PATTERN = /^[a-z0-9-]{1,48}$/;

export class ComposeEvidenceStore {
  #rootDirectory;

  constructor(rootDirectory) {
    this.#rootDirectory = path.resolve(rootDirectory);
  }

  initialize(plan, harness = null) {
    this.#assertLane(plan.lane);
    const stateDirectory = this.stateDirectory(plan.lane);
    fs.mkdirSync(path.join(stateDirectory, 'stages'), { recursive: true, mode: 0o700 });
    this.#writeNew(path.join(stateDirectory, 'plan.json'), plan);
    if (harness) this.#writeNew(path.join(stateDirectory, 'harness.json'), harness);
  }

  loadPlan(lane) {
    this.#assertLane(lane);
    return this.#readJson(path.join(this.stateDirectory(lane), 'plan.json'));
  }

  loadHarness(lane) {
    this.#assertLane(lane);
    const filePath = path.join(this.stateDirectory(lane), 'harness.json');
    return fs.existsSync(filePath) ? this.#readJson(filePath) : null;
  }

  writeStage(lane, result) {
    this.#assertLane(lane);
    if (!STAGE_PATTERN.test(result.id)) throw new Error(`invalid stage id: ${result.id}`);
    const filename = `${String(result.sequence).padStart(2, '0')}-${result.id}.json`;
    this.#writeNew(path.join(this.stateDirectory(lane), 'stages', filename), result);
  }

  loadStages(lane) {
    const directory = path.join(this.stateDirectory(lane), 'stages');
    return fs.readdirSync(directory)
      .filter((name) => name.endsWith('.json'))
      .sort()
      .map((name) => this.#readJson(path.join(directory, name)));
  }

  writeSummary(lane, summary, junit) {
    const directory = this.laneDirectory(lane);
    fs.writeFileSync(path.join(directory, 'compose-stage-evidence-v1.json'),
      `${JSON.stringify(summary, null, 2)}\n`, { mode: 0o600 });
    fs.writeFileSync(path.join(directory, 'compose-stage-evidence-v1.xml'), junit,
      { mode: 0o600 });
  }

  laneDirectory(lane) {
    this.#assertLane(lane);
    return path.join(this.#rootDirectory, lane);
  }

  stateDirectory(lane) {
    return path.join(this.laneDirectory(lane), '.state');
  }

  #writeNew(filePath, value) {
    fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`, {
      encoding: 'utf8',
      flag: 'wx',
      mode: 0o600,
    });
  }

  #readJson(filePath) {
    return JSON.parse(fs.readFileSync(filePath, 'utf8'));
  }

  #assertLane(lane) {
    if (!LANE_PATTERN.test(lane)) throw new Error(`invalid Compose evidence lane: ${lane}`);
  }
}
