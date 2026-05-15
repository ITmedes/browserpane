<script lang="ts">
  import DisplayControlsPanel from '../presentation/DisplayControlsPanel.svelte';
  import type { AdminMessageFeedback } from '../presentation/admin-message-types';
  import { DisplaySettingsViewModelBuilder } from '../presentation/display-settings-view-model';
  import type {
    BrowserSessionConnectPreferences,
    BrowserSessionRenderBackend,
  } from '../session/browser-session-types';

  type DisplayControlsSurfaceProps = {
    readonly connected: boolean;
    readonly preferences: BrowserSessionConnectPreferences;
    readonly onPreferencesChange: (preferences: BrowserSessionConnectPreferences) => void;
  };

  let {
    connected,
    preferences,
    onPreferencesChange,
  }: DisplayControlsSurfaceProps = $props();
  let feedback = $state<AdminMessageFeedback | null>(null);
  const viewModel = $derived(DisplaySettingsViewModelBuilder.build({
    preferences,
    connected,
  }));

  function updatePreference(patch: Partial<BrowserSessionConnectPreferences>, message: string): void {
    onPreferencesChange({ ...preferences, ...patch });
    feedback = {
      variant: 'info',
      title: 'Display preference saved',
      message,
      testId: 'display-message',
    };
  }
</script>

<DisplayControlsPanel
  {viewModel}
  {feedback}
  onRenderBackendChange={(renderBackend: BrowserSessionRenderBackend) => updatePreference(
    { renderBackend },
    connected ? 'Render backend will apply after reconnect.' : 'Render backend will apply on the next connect.',
  )}
  onHiDpiChange={(hiDpi) => updatePreference(
    { hiDpi },
    connected ? 'HiDPI preference will apply after reconnect.' : 'HiDPI preference will apply on the next connect.',
  )}
  onScrollCopyChange={(scrollCopy) => updatePreference(
    { scrollCopy },
    connected ? 'Scroll-copy preference will apply after reconnect.' : 'Scroll-copy preference will apply on the next connect.',
  )}
/>
