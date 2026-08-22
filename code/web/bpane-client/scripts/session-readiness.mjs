export function isSessionApplicationReady(state, expectedSessionId = '') {
  return state?.connected === true
    && state?.applicationReady === true
    && (!expectedSessionId || state?.sessionId === expectedSessionId);
}

export function isSessionFileTransferReady(state, expectedSessionId = '') {
  return isSessionApplicationReady(state, expectedSessionId)
    && state?.capabilities?.fileTransfer === true;
}
