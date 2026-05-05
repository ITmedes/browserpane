<script lang="ts">
  import type { SessionFileResource } from '../api/control-types';
  import {
    formatSessionFileBytes,
    formatSessionFileSource,
    formatSessionFileTimestamp,
    shortSessionFileDigest,
  } from './session-file-format';

  type SessionFileCardProps = {
    readonly file: SessionFileResource;
    readonly downloadingFileId: string | null;
    readonly onDownload: (file: SessionFileResource) => void;
  };

  let { file, downloadingFileId, onDownload }: SessionFileCardProps = $props();
</script>

<article
  class="grid grid-cols-[minmax(0,1fr)_auto] gap-x-4 gap-y-2.5 rounded-[18px] border border-admin-ink/10 bg-admin-cream/72 p-4 max-[760px]:grid-cols-1"
  data-testid="session-files-row"
>
  <div class="flex min-w-0 flex-wrap items-baseline gap-3">
    <strong class="[overflow-wrap:anywhere]">{file.name}</strong>
    <span class="text-admin-ink/68">{formatSessionFileSource(file.source)}</span>
  </div>
  <div class="flex min-w-0 flex-wrap items-baseline gap-3 text-admin-ink/68">
    <span>{formatSessionFileBytes(file.byte_count)}</span>
    <span>{file.media_type ?? 'application/octet-stream'}</span>
    <span>{formatSessionFileTimestamp(file.created_at)}</span>
  </div>
  <div class="font-mono text-[0.82rem] text-admin-ink/68">sha256 {shortSessionFileDigest(file.sha256_hex)}</div>
  <button
    class="admin-button-primary"
    type="button"
    data-testid="session-file-download"
    disabled={Boolean(downloadingFileId)}
    onclick={() => onDownload(file)}
  >
    {downloadingFileId === file.id ? 'Downloading...' : 'Download'}
  </button>
</article>
