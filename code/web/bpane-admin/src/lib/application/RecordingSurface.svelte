<script lang="ts">
  import type { ControlClient } from '../api/control-client';
  import type { SessionResource } from '../api/control-types';
  import type { SessionRecordingPlaybackResource, SessionRecordingResource } from '../api/recording-types';
  import type { LiveBrowserSessionConnection } from '../session/browser-session-types';
  import RecordingPanel from '../presentation/RecordingPanel.svelte';
  import { RecordingViewModelBuilder } from '../presentation/recording-view-model';

  type RecordingSurfaceProps = {
    readonly controlClient: ControlClient; readonly session: SessionResource | null;
    readonly liveConnection: LiveBrowserSessionConnection | null;
    readonly refreshVersion: number;
  };

  let { controlClient, session, liveConnection, refreshVersion }: RecordingSurfaceProps = $props();
  let currentSessionId = $state<string | null>(null);
  let lastRefreshVersion = $state(0);
  let recordings = $state<readonly SessionRecordingResource[]>([]);
  let playback = $state<SessionRecordingPlaybackResource | null>(null);
  let libraryLoading = $state(false);
  let libraryLoaded = $state(false);
  let libraryError = $state<string | null>(null);
  let downloadingRecordingId = $state<string | null>(null);
  let downloadingPlayback = $state(false);
  let libraryRequest = 0;
  let recording = $state(false);
  let busy = $state(false);
  let autoDownload = $state(true);
  let lastBlob = $state<Blob | null>(null);
  let lastArtifactName = $state<string | null>(null);
  let error = $state<string | null>(null);
  const viewModel = $derived(RecordingViewModelBuilder.build({
    liveConnection,
    selectedSessionId: currentSessionId,
    recording,
    busy,
    lastArtifactName,
    error,
    recordings,
    playback,
    libraryLoading,
    libraryLoaded,
    libraryError,
    downloadingRecordingId,
    downloadingPlayback,
  }));

  $effect(() => {
    const nextSessionId = session?.id ?? null;
    if (nextSessionId === currentSessionId) {
      return;
    }
    currentSessionId = nextSessionId;
    recordings = []; playback = null; libraryLoaded = false; libraryError = null;
    libraryRequest += 1;
    if (nextSessionId) {
      void loadLibrary(nextSessionId);
    }
  });

  $effect(() => {
    if (refreshVersion === lastRefreshVersion) {
      return;
    }
    lastRefreshVersion = refreshVersion;
    if (currentSessionId) {
      void loadLibrary(currentSessionId);
    }
  });

  async function loadLibrary(sessionId = currentSessionId): Promise<void> {
    if (!sessionId) {
      return;
    }
    const requestId = ++libraryRequest;
    libraryLoading = true;
    libraryError = null;
    try {
      const [recordingList, nextPlayback] = await Promise.all([
        controlClient.listSessionRecordings(sessionId),
        controlClient.getSessionRecordingPlayback(sessionId),
      ]);
      if (currentSessionId === sessionId && libraryRequest === requestId) {
        recordings = recordingList.recordings;
        playback = nextPlayback;
        libraryLoaded = true;
      }
    } catch (loadError) {
      if (currentSessionId === sessionId && libraryRequest === requestId) {
        recordings = [];
        playback = null;
        libraryLoaded = true;
        libraryError = errorMessage(loadError);
      }
    } finally {
      if (currentSessionId === sessionId && libraryRequest === requestId) {
        libraryLoading = false;
      }
    }
  }

  async function downloadSegment(recordingId: string): Promise<void> {
    const segment = recordings.find((entry) => entry.id === recordingId);
    if (!segment) {
      return;
    }
    downloadingRecordingId = recordingId;
    libraryError = null;
    try {
      const blob = await controlClient.downloadSessionRecordingContent(segment);
      saveBlob(blob, `browserpane-${segment.session_id}-${segment.id}.webm`);
    } catch (downloadError) {
      libraryError = errorMessage(downloadError);
    } finally {
      downloadingRecordingId = null;
    }
  }

  async function downloadPlaybackExport(): Promise<void> {
    if (!playback) {
      return;
    }
    downloadingPlayback = true;
    libraryError = null;
    try {
      const blob = await controlClient.downloadSessionRecordingPlaybackExport(playback);
      saveBlob(blob, `browserpane-${playback.session_id}-recording-playback.zip`);
    } catch (downloadError) {
      libraryError = errorMessage(downloadError);
    } finally {
      downloadingPlayback = false;
    }
  }

  async function startRecording(): Promise<void> {
    if (!liveConnection?.handle.startRecording) {
      return;
    }
    busy = true;
    error = null;
    try {
      await liveConnection.handle.startRecording({ frameRate: 24 });
      recording = true;
    } catch (startError) {
      error = errorMessage(startError);
    } finally {
      busy = false;
    }
  }

  async function stopRecording(): Promise<void> {
    if (!liveConnection?.handle.stopRecording) {
      return;
    }
    busy = true;
    error = null;
    try {
      lastBlob = await liveConnection.handle.stopRecording();
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
    if (lastBlob && lastArtifactName) {
      saveBlob(lastBlob, lastArtifactName);
    }
  }

  function saveBlob(blob: Blob, fileName: string): void {
    const url = URL.createObjectURL(blob);
    try {
      const link = document.createElement('a');
      link.href = url;
      link.download = fileName;
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
  onRefreshLibrary={() => void loadLibrary()}
  onDownloadSegment={(recordingId) => void downloadSegment(recordingId)}
  onDownloadPlayback={() => void downloadPlaybackExport()}
/>
