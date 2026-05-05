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

<article class="file-card" data-testid="session-files-row">
  <div class="file-title">
    <strong>{file.name}</strong>
    <span>{formatSessionFileSource(file.source)}</span>
  </div>
  <div class="meta">
    <span>{formatSessionFileBytes(file.byte_count)}</span>
    <span>{file.media_type ?? 'application/octet-stream'}</span>
    <span>{formatSessionFileTimestamp(file.created_at)}</span>
  </div>
  <div class="digest">sha256 {shortSessionFileDigest(file.sha256_hex)}</div>
  <button
    type="button"
    data-testid="session-file-download"
    disabled={Boolean(downloadingFileId)}
    onclick={() => onDownload(file)}
  >
    {downloadingFileId === file.id ? 'Downloading...' : 'Download'}
  </button>
</article>

<style>
  .file-card {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 10px 16px;
    padding: 16px;
    border: 1px solid rgba(24, 32, 24, 0.1);
    border-radius: 18px;
    background: rgba(255, 253, 243, 0.72);
  }

  .file-title,
  .meta {
    display: flex;
    min-width: 0;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 12px;
  }

  .file-title strong {
    overflow-wrap: anywhere;
  }

  .file-title span,
  .meta,
  .digest {
    color: rgba(24, 32, 24, 0.68);
  }

  .digest {
    font-family: "SFMono-Regular", Consolas, monospace;
    font-size: 0.82rem;
  }

  button {
    min-height: 40px;
    padding: 0 14px;
    border: 1px solid rgba(24, 32, 24, 0.18);
    border-radius: 999px;
    background: #243126;
    color: #fffdf3;
    font: inherit;
    font-weight: 800;
    cursor: pointer;
  }

  button:disabled {
    cursor: not-allowed;
    opacity: 0.45;
  }

  @media (max-width: 760px) {
    .file-card {
      grid-template-columns: 1fr;
    }
  }
</style>
