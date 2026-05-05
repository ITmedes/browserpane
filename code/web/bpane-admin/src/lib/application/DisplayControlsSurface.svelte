<script lang="ts">
  import DisplayControlsPanel from '../presentation/DisplayControlsPanel.svelte';
  import { DisplaySettingsViewModelBuilder } from '../presentation/display-settings-view-model';
  import type {
    BrowserSessionConnectPreferences,
    BrowserSessionRenderBackend,
    LiveBrowserSessionConnection,
  } from '../session/browser-session-types';

  type DisplayAction = 'microphone' | 'camera' | 'upload';

  type DisplayControlsSurfaceProps = {
    readonly liveConnection: LiveBrowserSessionConnection | null;
    readonly connected: boolean;
    readonly preferences: BrowserSessionConnectPreferences;
    readonly onPreferencesChange: (preferences: BrowserSessionConnectPreferences) => void;
  };

  let {
    liveConnection,
    connected,
    preferences,
    onPreferencesChange,
  }: DisplayControlsSurfaceProps = $props();
  let microphoneActive = $state(false);
  let cameraActive = $state(false);
  let busyAction = $state<DisplayAction | null>(null);
  let error = $state<string | null>(null);
  const viewModel = $derived(DisplaySettingsViewModelBuilder.build({
    preferences,
    connected,
    microphoneAvailable: Boolean(liveConnection?.handle.startMicrophone && liveConnection?.handle.stopMicrophone),
    cameraAvailable: Boolean(liveConnection?.handle.startCamera && liveConnection?.handle.stopCamera),
    uploadAvailable: Boolean(liveConnection?.handle.uploadFiles),
    microphoneActive,
    cameraActive,
    busy: Boolean(busyAction),
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

  function updatePreference(patch: Partial<BrowserSessionConnectPreferences>): void {
    onPreferencesChange({ ...preferences, ...patch });
  }

  async function toggleMicrophone(): Promise<void> {
    const handle = liveConnection?.handle;
    if (!handle) {
      return;
    }
    if (microphoneActive) {
      await runLiveAction('microphone', async () => {
        handle.stopMicrophone?.();
        microphoneActive = false;
      });
      return;
    }
    const startMicrophone = handle.startMicrophone;
    if (!startMicrophone) {
      error = 'The connected browser handle does not expose microphone control.';
      return;
    }
    await runLiveAction('microphone', async () => {
      await startMicrophone();
      microphoneActive = true;
    });
  }

  async function toggleCamera(): Promise<void> {
    const handle = liveConnection?.handle;
    if (!handle) {
      return;
    }
    if (cameraActive) {
      await runLiveAction('camera', async () => {
        handle.stopCamera?.();
        cameraActive = false;
      });
      return;
    }
    const startCamera = handle.startCamera;
    if (!startCamera) {
      error = 'The connected browser handle does not expose camera control.';
      return;
    }
    await runLiveAction('camera', async () => {
      await startCamera();
      cameraActive = true;
    });
  }

  async function uploadFiles(files: FileList): Promise<void> {
    const upload = liveConnection?.handle.uploadFiles;
    if (!upload) {
      error = 'The connected browser handle does not expose file upload.';
      return;
    }
    await runLiveAction('upload', async () => upload(files));
  }

  async function runLiveAction(action: DisplayAction, operation: () => Promise<void>): Promise<void> {
    busyAction = action;
    error = null;
    try {
      await operation();
    } catch (actionError) {
      error = actionError instanceof Error ? actionError.message : 'Display control failed';
    } finally {
      busyAction = null;
    }
  }
</script>

<DisplayControlsPanel
  {viewModel}
  onRenderBackendChange={(renderBackend: BrowserSessionRenderBackend) => updatePreference({ renderBackend })}
  onHiDpiChange={(hiDpi) => updatePreference({ hiDpi })}
  onScrollCopyChange={(scrollCopy) => updatePreference({ scrollCopy })}
  onMicrophoneToggle={() => void toggleMicrophone()}
  onCameraToggle={() => void toggleCamera()}
  onUploadFiles={(files) => void uploadFiles(files)}
/>
