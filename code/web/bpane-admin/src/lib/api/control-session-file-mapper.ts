import type { SessionFileListResponse, SessionFileResource } from './control-types';
import {
  expectNumber,
  expectRecord,
  expectString,
  expectStringRecord,
  optionalString,
} from './control-wire';

export class ControlSessionFileMapper {
  static toSessionFileList(payload: unknown): SessionFileListResponse {
    const object = expectRecord(payload, 'session file list response');
    const files = object.files;
    if (!Array.isArray(files)) {
      throw new Error('session file list response must contain a files array');
    }
    return {
      files: files.map((file) => this.toSessionFileResource(file)),
    };
  }

  static toSessionFileResource(payload: unknown): SessionFileResource {
    const object = expectRecord(payload, 'session file resource');
    const mediaType = optionalString(object.media_type, 'session file media_type');
    return {
      id: expectString(object.id, 'session file id'),
      session_id: expectString(object.session_id, 'session file session_id'),
      name: expectString(object.name, 'session file name'),
      ...(mediaType !== undefined ? { media_type: mediaType } : {}),
      byte_count: expectNumber(object.byte_count, 'session file byte_count'),
      sha256_hex: expectString(object.sha256_hex, 'session file sha256_hex'),
      source: expectString(object.source, 'session file source'),
      labels: expectStringRecord(object.labels, 'session file labels'),
      content_path: expectString(object.content_path, 'session file content_path'),
      created_at: expectString(object.created_at, 'session file created_at'),
      updated_at: expectString(object.updated_at, 'session file updated_at'),
    };
  }
}
