<script lang="ts">
  import { onMount } from 'svelte';
  import { adminErrorMessage } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { RecordingCatalogClient } from '$lib/recordings/recording-client';
  import {
    isDownloadableRecording,
    recordingOverviewRow,
    type RecordingActionState,
    type RecordingOverviewLoadState,
  } from '$lib/recordings/recording-overview-view-model';
  import type { RecordingCatalogEntry } from '$lib/recordings/recording-types';
  import { SessionCatalogClient } from '$lib/sessions/session-client';
  import RecordingOverview from './RecordingOverview.svelte';

  type RecordingOverviewRouteProps = {
    readonly authContext: UnifiedAdminContext;
  };

  let { authContext }: RecordingOverviewRouteProps = $props();
  let recordingState = $state<RecordingOverviewLoadState>({ status: 'loading' });
  let actionState = $state<RecordingActionState>({ status: 'idle' });

  onMount(() => {
    void loadRecordings();
  });

  function sessionClient(): SessionCatalogClient {
    return new SessionCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  function recordingClient(): RecordingCatalogClient {
    return new RecordingCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  async function loadRecordings(): Promise<void> {
    recordingState = { status: 'loading' };
    actionState = { status: 'idle' };
    try {
      const sessions = await sessionClient().listSessions();
      const response = await recordingClient().listRecordingsForSessions(sessions.sessions);
      recordingState = {
        status: 'ready',
        entries: response.entries,
        failures: response.failures,
      };
    } catch (error) {
      recordingState = {
        status: 'error',
        message: adminErrorMessage(error, 'Unexpected recording catalog error.'),
      };
    }
  }

  async function downloadRecording(entry: RecordingCatalogEntry): Promise<void> {
    const row = recordingOverviewRow(entry, downloadableSegmentCount(entry.session.id));
    actionState = { status: 'running', label: `Downloading ${row.shortId}...` };
    try {
      const blob = row.downloadKind === 'playback_export'
        ? await recordingClient().downloadSessionPlaybackExport(entry.session.id)
        : await recordingClient().downloadRecordingContent(entry.recording);
      triggerDownload(blob, row.downloadFileName);
      actionState = { status: 'success', message: `Download started for ${row.shortSessionId}.` };
    } catch (error) {
      actionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Recording download failed.'),
      };
    }
  }

  function downloadableSegmentCount(sessionId: string): number {
    if (recordingState.status !== 'ready') {
      return 0;
    }
    return recordingState.entries.filter((entry) =>
      entry.session.id === sessionId && isDownloadableRecording(entry.recording)).length;
  }

  function triggerDownload(blob: Blob, fileName: string): void {
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
</script>

<RecordingOverview
  state={recordingState}
  {actionState}
  onRefresh={loadRecordings}
  onDownloadRecording={downloadRecording}
/>
