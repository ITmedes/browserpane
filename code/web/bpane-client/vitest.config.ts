import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'jsdom',
    include: ['js/__tests__/**/*.test.ts'],
    globals: true,
    coverage: {
      provider: 'v8',
      include: ['js/**/*.{ts,js}'],
      reporter: ['text', 'json-summary'],
      reportsDirectory: 'coverage',
    },
  },
});
