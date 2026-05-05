import { describe, expect, it } from 'vitest';
import { AdminLogsViewModelBuilder } from './logs-view-model';

describe('AdminLogsViewModelBuilder', () => {
  it('disables log actions while the timeline is empty', () => {
    const viewModel = AdminLogsViewModelBuilder.build([]);

    expect(viewModel.countLabel).toBe('0 events');
    expect(viewModel.canCopy).toBe(false);
    expect(viewModel.canClear).toBe(false);
  });

  it('enables log actions when entries exist', () => {
    const viewModel = AdminLogsViewModelBuilder.build([{
      id: 'entry-1',
      timestamp: '12:00:00',
      level: 'info',
      message: 'Selected session session-a',
    }]);

    expect(viewModel.countLabel).toBe('1 events');
    expect(viewModel.canCopy).toBe(true);
    expect(viewModel.canClear).toBe(true);
  });
});
