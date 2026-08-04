import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import { adminCsp } from '@browserpane/admin-auth/svelte-csp-config';

const config = {
  preprocess: vitePreprocess(),
  kit: {
    csp: adminCsp,
    adapter: adapter({
      fallback: 'index.html',
    }),
    paths: {
      base: process.env.BPANE_ADMIN_BASE_PATH ?? '',
    },
  },
};

export default config;
