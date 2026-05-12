export function load({ params }: { params: { run_id: string } }): { runId: string } {
  return {
    runId: params.run_id,
  };
}
