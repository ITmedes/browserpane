export type AdminLogEntry = {
  readonly id: string;
  readonly timestamp: string;
  readonly level: 'info' | 'warn';
  readonly message: string;
};

export type AdminLogsViewModel = {
  readonly countLabel: string;
  readonly emptyLabel: string;
  readonly entries: readonly AdminLogEntry[];
  readonly canCopy: boolean;
  readonly canClear: boolean;
};

export class AdminLogsViewModelBuilder {
  static build(entries: readonly AdminLogEntry[]): AdminLogsViewModel {
    return {
      countLabel: `${entries.length} events`,
      emptyLabel: 'No admin events captured yet.',
      entries,
      canCopy: entries.length > 0,
      canClear: entries.length > 0,
    };
  }
}
