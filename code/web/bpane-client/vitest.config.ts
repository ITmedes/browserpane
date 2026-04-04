import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'jsdom',
    include: ['js/__tests__/**/*.test.ts'],
    globals: true,
  },
});
