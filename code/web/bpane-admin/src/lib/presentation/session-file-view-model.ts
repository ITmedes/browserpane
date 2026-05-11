import type { SessionFileResource } from '../api/control-types';
import {
  formatSessionFileBytes,
  formatSessionFileSource,
  formatSessionFileTimestamp,
  shortSessionFileDigest,
} from './session-file-format';

export type SessionFileCardViewModel = {
  readonly id: string;
  readonly name: string;
  readonly source: string;
  readonly size: string;
  readonly mediaType: string;
  readonly createdAt: string;
  readonly digest: string;
};

export class SessionFileViewModelBuilder {
  static card(file: SessionFileResource): SessionFileCardViewModel {
    return {
      id: file.id,
      name: file.name,
      source: formatSessionFileSource(file.source),
      size: formatSessionFileBytes(file.byte_count),
      mediaType: file.media_type ?? 'application/octet-stream',
      createdAt: formatSessionFileTimestamp(file.created_at),
      digest: `sha256 ${shortSessionFileDigest(file.sha256_hex)}`,
    };
  }
}
