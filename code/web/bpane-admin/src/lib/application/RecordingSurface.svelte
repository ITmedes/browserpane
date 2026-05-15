<script lang="ts">
  import { onMount } from 'svelte';
  import type { ControlClient } from '../api/control-client';
  import type { SessionResource } from '../api/control-types';
  import type { SessionRecordingPlaybackResource, SessionRecordingResource } from '../api/recording-types';
  import type { LiveBrowserSessionConnection } from '../session/browser-session-types';
  import type { AdminMessageFeedback } from '../presentation/admin-message-types';
  import RecordingPanel from '../presentation/RecordingPanel.svelte';
  import { RecordingViewModelBuilder } from '../presentation/recording-view-model';
  import { saveBlob } from './recording-downloads';

  const LIBRARY_RECONCILE_INTERVAL_MS = 2_500;

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
  let feedback = $state<AdminMessageFeedback | null>(null);
  const activeConnection = $derived(liveConnection?.sessionId === currentSessionId ? liveConnection : null);
  const viewModel = $derived(RecordingViewModelBuilder.build({
    liveConnection: activeConnection,
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

  onMount(() => {
    const timer = window.setInterval(() => {
      if (shouldReconcileLibrary()) void loadLibrary(currentSessionId, false);
    }, LIBRARY_RECONCILE_INTERVAL_MS);
    return () => window.clearInterval(timer);
  });

  $effect(() => {
    const nextSessionId = session?.id ?? null;
    if (nextSessionId === currentSessionId) {
      return;
    }
    currentSessionId = nextSessionId;
    recordings = [];
    playback = null;
    libraryLoading = false;
    libraryLoaded = false;
    libraryError = null;
    downloadingRecordingId = null;
    downloadingPlayback = false;
    recording = false;
    busy = false;
    lastBlob = null;
    lastArtifactName = null;
    error = null;
    feedback = null;
    libraryRequest += 1;
    if (nextSessionId) {
      void loadLibrary(nextSessionId, false);
    }
  });

  $effect(() => {
    if (refreshVersion === lastRefreshVersion) {
      return;
    }
    lastRefreshVersion = refreshVersion;
    if (currentSessionId) {
      void loadLibrary(currentSessionId, false);
    }
  });
  $effect(() => { recording = activeConnection?.handle.isRecording?.() ?? false; });

  async function loadLibrary(sessionId = currentSessionId, showFeedback = true): Promise<void> {
    if (!sessionId) {
      return;
    }
    const requestId = ++libraryRequest;
    libraryLoading = true;
    libraryError = null;
    if (showFeedback) {
      feedback = null;
    }
    try {
      const [recordingList, nextPlayback] = await Promise.all([
        controlClient.listSessionRecordings(sessionId),
        controlClient.getSessionRecordingPlayback(sessionId),
      ]);
      if (currentSessionId === sessionId && libraryRequest === requestId) {
        recordings = recordingList.recordings;
        playback = nextPlayback;
        libraryLoaded = true;
        if (showFeedback) {
          feedback = successFeedback(`Recording library refreshed with ${recordingList.recordings.length} segment${recordingList.recordings.length === 1 ? '' : 's'}.`);
        }
      }
    } catch (loadError) {
      if (currentSessionId === sessionId && libraryRequest === requestId) {
        recordings = [];
        playback = null;
        libraryLoaded = true;
        libraryError = errorMessage(loadError);
        feedback = null;
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
    feedback = null;
    try {
      const blob = await controlClient.downloadSessionRecordingContent(segment);
      saveBlob(blob, `browserpane-${segment.session_id}-${segment.id}.webm`);
      feedback = successFeedback('Recording segment download started.');
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
    feedback = null;
    try {
      const blob = await controlClient.downloadSessionRecordingPlaybackExport(playback);
      saveBlob(blob, `browserpane-${playback.session_id}-recording-playback.zip`);
      feedback = successFeedback('Session recording export download started.');
    } catch (downloadError) {
      libraryError = errorMessage(downloadError);
    } finally {
      downloadingPlayback = false;
    }
  }

  async function startRecording(): Promise<void> {
    if (!activeConnection?.handle.startRecording) {
      return;
    }
    busy = true;
    error = null;
    feedback = null;
    try {
      await activeConnection.handle.startRecording({ frameRate: 24 });
      recording = true;
      feedback = successFeedback('Recording started for the selected browser session.');
    } catch (startError) {
      error = errorMessage(startError);
    } finally {
      busy = false;
    }
  }

  async function stopRecording(): Promise<void> {
    if (!activeConnection?.handle.stopRecording) {
      return;
    }
    busy = true;
    error = null;
    feedback = null;
    try {
      lastBlob = await activeConnection.handle.stopRecording();
      lastArtifactName = `bpane-${activeConnection.sessionId}-${Date.now()}.webm`;
      recording = false;
      feedback = successFeedback('Recording stopped. The latest WebM is ready.');
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
      feedback = successFeedback('Latest recording download started.');
    }
  }

  function setAutoDownload(enabled: boolean): void {
    autoDownload = enabled;
    feedback = {
      variant: 'info',
      title: 'Recording preference updated',
      message: enabled ? 'Auto download is enabled.' : 'Auto download is disabled.',
      testId: 'recording-message',
    };
  }

  function shouldReconcileLibrary(): boolean { return Boolean(currentSessionId && !libraryLoading && !downloadingRecordingId && !downloadingPlayback); }

  function errorMessage(value: unknown): string { return value instanceof Error ? value.message : 'Recording operation failed'; }

  function successFeedback(message: string): AdminMessageFeedback {
    return { variant: 'success', title: 'Recording updated', message, testId: 'recording-message' };
  }
</script>

<RecordingPanel {viewModel} {autoDownload} {feedback}
  onAutoDownloadChange={setAutoDownload}
  onStart={() => void startRecording()} onStop={() => void stopRecording()} onDownload={downloadLast}
  onRefreshLibrary={() => void loadLibrary()} onDownloadSegment={(recordingId) => void downloadSegment(recordingId)}
  onDownloadPlayback={() => void downloadPlaybackExport()} />
