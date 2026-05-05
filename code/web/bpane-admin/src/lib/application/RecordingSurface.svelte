<script lang="ts">
  import type { LiveBrowserSessionConnection } from '../session/browser-session-types';
  import RecordingPanel from '../presentation/RecordingPanel.svelte';
  import { RecordingViewModelBuilder } from '../presentation/recording-view-model';

  type RecordingSurfaceProps = {
    readonly liveConnection: LiveBrowserSessionConnection | null;
  };

  let { liveConnection }: RecordingSurfaceProps = $props();
  let recording = $state(false);
  let busy = $state(false);
  let autoDownload = $state(true);
  let lastBlob = $state<Blob | null>(null);
  let lastArtifactName = $state<string | null>(null);
  let error = $state<string | null>(null);
  const viewModel = $derived(RecordingViewModelBuilder.build({
    liveConnection,
    recording,
    busy,
    lastArtifactName,
    error,
  }));

  async function startRecording(): Promise<void> {
    const start = liveConnection?.handle.startRecording;
    if (!start) {
      return;
    }
    busy = true;
    error = null;
    try {
      await start({ frameRate: 24 });
      recording = true;
    } catch (startError) {
      error = errorMessage(startError);
    } finally {
      busy = false;
    }
  }

  async function stopRecording(): Promise<void> {
    const stop = liveConnection?.handle.stopRecording;
    if (!stop) {
      return;
    }
    busy = true;
    error = null;
    try {
      lastBlob = await stop();
      lastArtifactName = `bpane-${liveConnection?.sessionId ?? 'session'}-${Date.now()}.webm`;
      recording = false;
      if (autoDownload) {
        downloadLast();
      }
    } catch (stopError) {
      error = errorMessage(stopError);
    } finally {
      busy = false;
    }
  }

  function downloadLast(): void {
    if (!lastBlob || !lastArtifactName) {
      return;
    }
    const url = URL.createObjectURL(lastBlob);
    try {
      const link = document.createElement('a');
      link.href = url;
      link.download = lastArtifactName;
      document.body.append(link);
      link.click();
      link.remove();
    } finally {
      URL.revokeObjectURL(url);
    }
  }

  function errorMessage(value: unknown): string {
    return value instanceof Error ? value.message : 'Recording operation failed';
  }
</script>

<RecordingPanel
  {viewModel}
  {autoDownload}
  onAutoDownloadChange={(enabled) => { autoDownload = enabled; }}
  onStart={() => void startRecording()}
  onStop={() => void stopRecording()}
  onDownload={downloadLast}
/>
