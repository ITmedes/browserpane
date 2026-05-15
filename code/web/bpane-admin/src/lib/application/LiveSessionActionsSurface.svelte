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
    if (!connected) {
      microphoneActive = false;
      cameraActive = false;
      busyAction = null;
      error = null;
      feedback = null;
    }
  });

  async function toggleMicrophone(): Promise<void> {
    const handle = liveConnection?.handle;
    if (!handle) return;
    if (microphoneActive) {
      await runLiveAction('microphone', 'Microphone stopped.', async () => {
        handle.stopMicrophone?.();
        microphoneActive = false;
      });
      return;
    }
    if (!handle.startMicrophone) {
      error = 'The connected browser handle does not expose microphone control.';
      feedback = null;
      return;
    }
    await runLiveAction('microphone', 'Microphone started.', async () => {
      await handle.startMicrophone?.();
      microphoneActive = true;
    });
  }

  async function toggleCamera(): Promise<void> {
    const handle = liveConnection?.handle;
    if (!handle) return;
    if (cameraActive) {
      await runLiveAction('camera', 'Camera stopped.', async () => {
        handle.stopCamera?.();
        cameraActive = false;
      });
      return;
    }
    if (!handle.startCamera) {
      error = 'The connected browser handle does not expose camera control.';
      feedback = null;
      return;
    }
    await runLiveAction('camera', 'Camera started.', async () => {
      await handle.startCamera?.();
      cameraActive = true;
    });
  }

  async function uploadFiles(files: FileList): Promise<void> {
    const handle = liveConnection?.handle;
    if (!handle?.uploadFiles) {
      error = 'The connected browser handle does not expose file upload.';
      feedback = null;
      return;
    }
    const count = files.length;
    await runLiveAction(
      'upload',
      `${count} ${count === 1 ? 'file was' : 'files were'} sent to the session.`,
      async () => handle.uploadFiles?.(files),
    );
  }

  async function runLiveAction(
    action: LiveSessionActionId,
    successMessage: string,
    operation: () => Promise<void>,
  ): Promise<void> {
    busyAction = action;
    error = null;
    feedback = null;
    try {
      await operation();
      feedback = { variant: 'success', title: 'Operation complete', message: successMessage, testId: 'display-message' };
    } catch (actionError) {
      error = actionError instanceof Error ? actionError.message : 'Live session action failed';
      feedback = null;
    } finally {
      busyAction = null;
    }
  }
</script>

<LiveSessionActionsPanel
  {viewModel}
  {feedback}
  onCameraToggle={() => void toggleCamera()}
  onMicrophoneToggle={() => void toggleMicrophone()}
  onUploadFiles={(files) => void uploadFiles(files)}
/>
