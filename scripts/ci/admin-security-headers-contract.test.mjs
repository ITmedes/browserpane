import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const NGINX_CONFIG = new URL('../../deploy/nginx.conf', import.meta.url);
const HEADER_CONFIG = new URL('../../deploy/nginx-admin-security-headers.conf', import.meta.url);
const DOCKERFILE = new URL('../../deploy/Dockerfile.web', import.meta.url);
const COMPAT_CONFIG = new URL('../../code/web/bpane-admin/svelte.config.js', import.meta.url);
const NEW_CONFIG = new URL('../../code/web/bpane-admin-unified/svelte.config.js', import.meta.url);
const INCLUDE = 'include /etc/nginx/snippets/admin-security-headers.conf;';

test('admin routes and the promoted root share the hardened response policy', async () => {
  const nginx = await readFile(NGINX_CONFIG, 'utf8');

  for (const route of ['/', '/admin', '/admin/', '/admin-new', '/admin-new/']) {
    assert.match(locationBlock(nginx, route), new RegExp(escapeRegExp(INCLUDE)));
  }
});

test('web root selects admin-new while preserving explicit compatibility and fixture paths', async () => {
  const nginx = await readFile(NGINX_CONFIG, 'utf8');

  assert.match(locationBlock(nginx, '/'), /return 302 \/admin-new\//);
  assert.doesNotMatch(locationBlock(nginx, '/admin/'), /return 302 \/admin-new\//);
  assert.match(nginx, /location \/ \{[\s\S]*?try_files \$uri \$uri\/ =404;/);
});

test('admin security policy contains the required browser defenses', async () => {
  const headers = await readFile(HEADER_CONFIG, 'utf8');

  assert.match(headers, /Content-Security-Policy/);
  assert.match(headers, /frame-ancestors 'none'/);
  assert.match(headers, /object-src 'none'/);
  assert.match(headers, /X-Content-Type-Options "nosniff" always/);
  assert.match(headers, /X-Frame-Options "DENY" always/);
  assert.match(headers, /Referrer-Policy "no-referrer" always/);
  assert.match(headers, /Permissions-Policy/);
});

test('both static apps use the shared hash-based CSP', async () => {
  for (const configUrl of [COMPAT_CONFIG, NEW_CONFIG]) {
    const config = await readFile(configUrl, 'utf8');
    assert.match(config, /@browserpane\/admin-auth\/svelte-csp-config/);
    assert.match(config, /csp: adminCsp/);
  }
  const { adminCsp } = await import('../../code/web/bpane-admin-auth/src/svelte-csp-config.js');
  assert.equal(adminCsp.mode, 'hash');
  assert.deepEqual(adminCsp.directives['script-src'], ['self']);
  assert.deepEqual(adminCsp.directives['object-src'], ['none']);
});

test('web image packages the shared admin header policy', async () => {
  const dockerfile = await readFile(DOCKERFILE, 'utf8');

  assert.match(
    dockerfile,
    /COPY deploy\/nginx-admin-security-headers\.conf \/etc\/nginx\/snippets\/admin-security-headers\.conf/,
  );
});

function locationBlock(config, route) {
  const escaped = escapeRegExp(route);
  const pattern = route.endsWith('/') && route !== '/'
    ? new RegExp(`location \\^~ ${escaped} \\{([\\s\\S]*?)\\n    \\}`)
    : new RegExp(`location = ${escaped} \\{([\\s\\S]*?)\\n    \\}`);
  const match = config.match(pattern);
  assert.ok(match, `missing nginx location for ${route}`);
  return match[1];
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
