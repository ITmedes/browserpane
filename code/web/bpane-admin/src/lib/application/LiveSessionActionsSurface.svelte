<script lang="ts">
  import LiveSessionActionsPanel from '../presentation/LiveSessionActionsPanel.svelte';
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
    }
  });

  async function toggleMicrophone(): Promise<void> {
    const handle = liveConnection?.handle;
    if (!handle) return;
    if (microphoneActive) {
      await runLiveAction('microphone', async () => {
        handle.stopMicrophone?.();
        microphoneActive = false;
      });
      return;
    }
    if (!handle.startMicrophone) {
      error = 'The connected browser handle does not expose microphone control.';
      return;
    }
    await runLiveAction('microphone', async () => {
      await handle.startMicrophone?.();
      microphoneActive = true;
    });
  }

  async function toggleCamera(): Promise<void> {
    const handle = liveConnection?.handle;
    if (!handle) return;
    if (cameraActive) {
      await runLiveAction('camera', async () => {
        handle.stopCamera?.();
        cameraActive = false;
      });
      return;
    }
    if (!handle.startCamera) {
      error = 'The connected browser handle does not expose camera control.';
      return;
    }
    await runLiveAction('camera', async () => {
      await handle.startCamera?.();
      cameraActive = true;
    });
  }

  async function uploadFiles(files: FileList): Promise<void> {
    const handle = liveConnection?.handle;
    if (!handle?.uploadFiles) {
      error = 'The connected browser handle does not expose file upload.';
      return;
    }
    await runLiveAction('upload', async () => handle.uploadFiles?.(files));
  }

  async function runLiveAction(action: LiveSessionActionId, operation: () => Promise<void>): Promise<void> {
    busyAction = action;
    error = null;
    try {
      await operation();
    } catch (actionError) {
      error = actionError instanceof Error ? actionError.message : 'Live session action failed';
    } finally {
      busyAction = null;
    }
  }
</script>

<LiveSessionActionsPanel
  {viewModel}
  onCameraToggle={() => void toggleCamera()}
  onMicrophoneToggle={() => void toggleMicrophone()}
  onUploadFiles={(files) => void uploadFiles(files)}
/>
