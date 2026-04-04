import { defineConfig } from 'vite';
import { resolve } from 'path';

export default defineConfig({
  build: {
    lib: {
      entry: resolve(__dirname, 'js/bpane.ts'),
      formats: ['es'],
      fileName: 'bpane',
    },
    outDir: 'dist',
    emptyOutDir: true,
    sourcemap: true,
  },
});
