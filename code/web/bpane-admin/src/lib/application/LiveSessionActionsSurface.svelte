<script lang="ts">
  import LiveSessionActionsPanel from '../presentation/LiveSessionActionsPanel.svelte';
  import type { AdminMessageFeedback } from '../presentation/admin-message-types';
  import {
    type LiveSessionActionId,
    LiveSessionActionsViewModelBuilder,
  } from '../presentation/live-session-actions-view-model';
  import type { LiveBrowserSessionConnection } from '../session/browser-session-types';

  type LiveSessionActionsSurfaceProps = {
    readonly liveConnection: LiveBrowserSessionConnection | null;
    readonly connected: boolean;
  };

  let { liveConnection, connected }: LiveSessionActionsSurfaceProps = $props();
  let microphoneActive = $state(false);
  let cameraActive = $state(false);
  let busyAction = $state<LiveSessionActionId | null>(null);
  let error = $state<string | null>(null);
  let feedback = $state<AdminMessageFeedback | null>(null);
  let observedConnectionSessionId = $state<string | null>(null);
  const currentConnectionSessionId = $derived(connected ? liveConnection?.sessionId ?? null : null);
  const viewModel = $derived(LiveSessionActionsViewModelBuilder.build({
    connected,
    cameraAvailable: Boolean(liveConnection?.handle.startCamera && liveConnection?.handle.stopCamera),
    microphoneAvailable: Boolean(liveConnection?.handle.startMicrophone && liveConnection?.handle.stopMicrophone),
    uploadAvailable: Boolean(liveConnection?.handle.uploadFiles),
    cameraActive,
    microphoneActive,
    busyAction,
    error,
  }));

  $effect(() => {
    if (currentConnectionSessionId === observedConnectionSessionId) {
      return;
    }
    observedConnectionSessionId = currentConnectionSessionId;
    microphoneActive = false;
    cameraActive = false;
    busyAction = null;
    error = null;
    feedback = null;
  });

  async function toggleMicrophone(): Promise<void> {
    const handle = liveConnection?.handle;
    const requestSessionId = currentConnectionSessionId;
    if (!handle || !requestSessionId) return;
    if (microphoneActive) {
      await runLiveAction(requestSessionId, 'microphone', 'Microphone stopped.', async () => {
        handle.stopMicrophone?.();
      }, () => {
        microphoneActive = false;
      });
      return;
    }
    if (!handle.startMicrophone) {
      showCurrentError(requestSessionId, 'The connected browser handle does not expose microphone control.');
      return;
    }
    await runLiveAction(requestSessionId, 'microphone', 'Microphone started.', async () => {
      await handle.startMicrophone?.();
    }, () => {
      microphoneActive = true;
    });
  }

  async function toggleCamera(): Promise<void> {
    const handle = liveConnection?.handle;
    const requestSessionId = currentConnectionSessionId;
    if (!handle || !requestSessionId) return;
    if (cameraActive) {
      await runLiveAction(requestSessionId, 'camera', 'Camera stopped.', async () => {
        handle.stopCamera?.();
      }, () => {
        cameraActive = false;
      });
      return;
    }
    if (!handle.startCamera) {
      showCurrentError(requestSessionId, 'The connected browser handle does not expose camera control.');
      return;
    }
    await runLiveAction(requestSessionId, 'camera', 'Camera started.', async () => {
      await handle.startCamera?.();
    }, () => {
      cameraActive = true;
    });
  }

  async function uploadFiles(files: FileList): Promise<void> {
    const handle = liveConnection?.handle;
    const requestSessionId = currentConnectionSessionId;
    if (!handle || !requestSessionId) {
      return;
    }
    if (!handle.uploadFiles) {
      showCurrentError(requestSessionId, 'The connected browser handle does not expose file upload.');
      return;
    }
    const count = files.length;
    await runLiveAction(
      requestSessionId,
      'upload',
      `${count} ${count === 1 ? 'file was' : 'files were'} sent to the session.`,
      async () => handle.uploadFiles?.(files),
    );
  }

  async function runLiveAction(
    sessionId: string,
    action: LiveSessionActionId,
    successMessage: string,
    operation: () => Promise<void>,
    applySuccess?: () => void,
  ): Promise<void> {
    busyAction = action;
    error = null;
    feedback = null;
    try {
      await operation();
      if (isCurrentLiveSession(sessionId)) {
        applySuccess?.();
        feedback = { variant: 'success', title: 'Operation complete', message: successMessage, testId: 'display-message' };
      }
    } catch (actionError) {
      if (isCurrentLiveSession(sessionId)) {
        error = actionError instanceof Error ? actionError.message : 'Live session action failed';
        feedback = null;
      }
    } finally {
      if (isCurrentLiveSession(sessionId)) {
        busyAction = null;
      }
    }
  }

  function showCurrentError(sessionId: string, message: string): void {
    if (isCurrentLiveSession(sessionId)) {
      error = message;
      feedback = null;
    }
  }

  function isCurrentLiveSession(sessionId: string): boolean {
    return currentConnectionSessionId === sessionId;
  }
</script>

<LiveSessionActionsPanel
  {viewModel}
  {feedback}
  onCameraToggle={() => void toggleCamera()}
  onMicrophoneToggle={() => void toggleMicrophone()}
  onUploadFiles={(files) => void uploadFiles(files)}
/>
