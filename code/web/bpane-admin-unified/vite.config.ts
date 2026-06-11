import tailwindcss from '@tailwindcss/vite';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';

export default defineConfig(({ mode }) => {
  const config = {
    plugins: [tailwindcss(), sveltekit()],
    test: {
      environment: 'jsdom' as const,
      include: ['src/**/*.test.ts'],
    },
  };

  if (mode === 'test') {
    return {
      ...config,
      resolve: { conditions: ['browser'] },
    };
  }

  return config;
});
