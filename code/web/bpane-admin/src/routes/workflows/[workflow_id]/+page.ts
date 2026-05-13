export function load({ params }: { params: { workflow_id: string } }): { workflowId: string } {
  return {
    workflowId: params.workflow_id,
  };
}
