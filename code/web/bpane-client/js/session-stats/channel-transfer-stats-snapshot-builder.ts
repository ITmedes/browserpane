import {
  CH_VIDEO, CH_AUDIO_OUT, CH_AUDIO_IN, CH_VIDEO_IN, CH_INPUT, CH_CURSOR,
  CH_CLIPBOARD, CH_CONTROL,
} from '../protocol.js';
import { CH_TILES } from '../tile-compositor.js';
import type { ChannelTransferStats } from './models.js';

export class ChannelTransferStatsSnapshotBuilder {
  static build(
    bytesByChannel: Record<number, number>,
    framesByChannel: Record<number, number>,
  ): Record<string, ChannelTransferStats> {
    const ids = new Set<number>([
      CH_VIDEO,
      CH_AUDIO_OUT,
      CH_AUDIO_IN,
      CH_VIDEO_IN,
      CH_INPUT,
      CH_CURSOR,
      CH_CLIPBOARD,
      CH_CONTROL,
      CH_TILES,
      ...Object.keys(bytesByChannel).map((key) => Number(key)),
      ...Object.keys(framesByChannel).map((key) => Number(key)),
    ]);
    const snapshot: Record<string, ChannelTransferStats> = {};

    for (const id of ids) {
      snapshot[ChannelTransferStatsSnapshotBuilder.labelFor(id)] = {
        bytes: bytesByChannel[id] ?? 0,
        frames: framesByChannel[id] ?? 0,
      };
    }

    return snapshot;
  }

  static labelFor(channelId: number): string {
    switch (channelId) {
      case CH_VIDEO: return 'video';
      case CH_AUDIO_OUT: return 'audioOut';
      case CH_AUDIO_IN: return 'audioIn';
      case CH_VIDEO_IN: return 'videoIn';
      case CH_INPUT: return 'input';
      case CH_CURSOR: return 'cursor';
      case CH_CLIPBOARD: return 'clipboard';
      case CH_CONTROL: return 'control';
      case CH_TILES: return 'tiles';
      default: return `ch${channelId}`;
    }
  }
}
