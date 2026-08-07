import fs from 'node:fs';

export class InventoryFile {
  static render(inventory) {
    return `${JSON.stringify(inventory, null, 2)}\n`;
  }

  static write(filename, inventory) {
    fs.writeFileSync(filename, this.render(inventory));
  }

  static check(filename, inventory) {
    const expected = this.render(inventory);
    const actual = fs.readFileSync(filename, 'utf8');
    if (actual !== expected) {
      throw new Error('generated operation inventory is stale; run npm run generate');
    }
  }
}
