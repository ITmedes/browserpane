<script lang="ts">
  import type { SessionResource } from '../api/control-types';
  import WorkflowOperationsPanel from '../presentation/WorkflowOperationsPanel.svelte';
  import { WorkflowOperationsViewModelBuilder } from '../presentation/workflow-operations-view-model';

  type WorkflowOperationsSurfaceProps = {
    readonly selectedSession: SessionResource | null;
  };

  let { selectedSession }: WorkflowOperationsSurfaceProps = $props();
  const viewModel = $derived(WorkflowOperationsViewModelBuilder.build({
    selectedSession,
    apiAvailable: false,
  }));
</script>

<WorkflowOperationsPanel {viewModel} />
