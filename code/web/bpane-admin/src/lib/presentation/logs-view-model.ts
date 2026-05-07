export type AdminLogEntry = {
  readonly id: string;
  readonly timestamp: string;
  readonly level: 'info' | 'warn';
  readonly source: 'gateway' | 'ui';
  readonly message: string;
};

export type AdminLogsViewModel = {
  readonly countLabel: string;
  readonly sourceLabel: string;
  readonly emptyLabel: string;
  readonly entries: readonly AdminLogEntry[];
  readonly canCopy: boolean;
  readonly canClear: boolean;
};

export class AdminLogsViewModelBuilder {
  static build(entries: readonly AdminLogEntry[]): AdminLogsViewModel {
    const gatewayCount = entries.filter((entry) => entry.source === 'gateway').length;
    const uiCount = entries.length - gatewayCount;
    return {
      countLabel: `${entries.length} events`,
      sourceLabel: `${gatewayCount} gateway / ${uiCount} local`,
      emptyLabel: 'No admin events captured yet.',
      entries,
      canCopy: entries.length > 0,
      canClear: entries.length > 0,
    };
  }
}
