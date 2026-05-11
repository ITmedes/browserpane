<script lang="ts">
  import DisplayControlsPanel from '../presentation/DisplayControlsPanel.svelte';
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
  const viewModel = $derived(DisplaySettingsViewModelBuilder.build({
    preferences,
    connected,
  }));

  function updatePreference(patch: Partial<BrowserSessionConnectPreferences>): void {
    onPreferencesChange({ ...preferences, ...patch });
  }
</script>

<DisplayControlsPanel
  {viewModel}
  onRenderBackendChange={(renderBackend: BrowserSessionRenderBackend) => updatePreference({ renderBackend })}
  onHiDpiChange={(hiDpi) => updatePreference({ hiDpi })}
  onScrollCopyChange={(scrollCopy) => updatePreference({ scrollCopy })}
/>
