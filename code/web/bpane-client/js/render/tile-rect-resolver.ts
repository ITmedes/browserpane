import type { TileGridConfig } from '../tile-cache.js';

export type ResolveTileRectArgs = {
  gridConfig: TileGridConfig | null;
  col: number;
  row: number;
  gridOffsetX: number;
  gridOffsetY: number;
  applyOffset: boolean;
};

export function resolveTileRect(args: ResolveTileRectArgs): { x: number; y: number; w: number; h: number } | null {
  const { gridConfig, col, row, gridOffsetX, gridOffsetY, applyOffset } = args;
  if (!gridConfig) return null;

  const tileSize = gridConfig.tileSize;
  const offsetX = applyOffset ? gridOffsetX : 0;
  const offsetY = applyOffset ? gridOffsetY : 0;
  const rawX = col * tileSize - offsetX;
  const rawY = row * tileSize - offsetY;
  const x = Math.max(0, rawX);
  const y = Math.max(0, rawY);
  const endX = Math.min(gridConfig.screenW, rawX + tileSize);
  const endY = Math.min(gridConfig.screenH, rawY + tileSize);
  const width = endX - x;
  const height = endY - y;

  if (width <= 0 || height <= 0) return null;
  return { x, y, w: width, h: height };
}
