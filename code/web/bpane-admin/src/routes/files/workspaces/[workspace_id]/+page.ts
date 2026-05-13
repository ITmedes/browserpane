export function load({ params }: { params: { workspace_id: string } }): { workspaceId: string } {
  return {
    workspaceId: params.workspace_id,
  };
}
