export function load({ params }: { params: { session_id: string } }): { sessionId: string } {
  return {
    sessionId: params.session_id,
  };
}
