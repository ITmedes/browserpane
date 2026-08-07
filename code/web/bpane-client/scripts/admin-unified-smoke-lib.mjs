import { apiOrigin, poll } from './workflow-smoke-lib.mjs';

export const ADMIN_UNIFIED_ROUTE_MANIFEST = Object.freeze([
  Object.freeze({ name: 'dashboard', path: '', readyTestId: 'dashboard-overview' }),
  Object.freeze({ name: 'projects', path: 'projects', readyTestId: 'projects-overview' }),
  Object.freeze({
    name: 'browser-contexts',
    path: 'browser-contexts',
    readyTestId: 'browser-contexts-overview',
  }),
  Object.freeze({ name: 'egress', path: 'egress', readyTestId: 'egress-profiles-overview' }),
  Object.freeze({
    name: 'file-workspaces',
    path: 'files/workspaces',
    readyTestId: 'file-workspaces-overview',
  }),
  Object.freeze({ name: 'workflows', path: 'workflows', readyTestId: 'workflows-overview' }),
  Object.freeze({
    name: 'workflow-runs',
    path: 'workflow-runs',
    readyTestId: 'workflow-runs-overview',
  }),
  Object.freeze({ name: 'sessions', path: 'sessions', readyTestId: 'sessions-overview' }),
  Object.freeze({ name: 'recordings', path: 'recordings', readyTestId: 'recordings-overview' }),
  Object.freeze({ name: 'extensions', path: 'extensions', readyTestId: 'extensions-overview' }),
  Object.freeze({
    name: 'credential-bindings',
    path: 'credential-bindings',
    readyTestId: 'credential-bindings-overview',
  }),
  Object.freeze({
    name: 'workflow-event-subscriptions',
    path: 'workflow-event-subscriptions',
    readyTestId: 'workflow-event-subscriptions-overview',
  }),
  Object.freeze({ name: 'identity', path: 'identity', readyTestId: 'identity-access-workspace' }),
  Object.freeze({ name: 'api', path: 'api', readyTestId: 'api-companion-workspace' }),
  Object.freeze({ name: 'coverage', path: 'coverage', readyTestId: 'api-coverage-workspace' }),
  Object.freeze({ name: 'docs', path: 'docs', readyTestId: 'admin-docs-workspace' }),
]);

export function adminRouteUrl(options, routePath) {
  const normalizedPath = routePath.replace(/^\/+|\/+$/g, '');
  const pathname = normalizedPath ? `/admin-new/${normalizedPath}` : '/admin-new/';
  return new URL(pathname, apiOrigin(options)).toString();
}

export function authJsonHeaders(accessToken) {
  return {
    Authorization: `Bearer ${accessToken}`,
    'content-type': 'application/json',
  };
}

export function findActiveSessionIdsByLabels(response, expectedLabels) {
  if (!response || !Array.isArray(response.sessions)) {
    throw new Error('Expected a session catalog response.');
  }
  return response.sessions.flatMap((session) => {
    if (
      !session ||
      typeof session.id !== 'string' ||
      !session.id ||
      typeof session.state !== 'string' ||
      session.state === 'stopped' ||
      !session.labels ||
      typeof session.labels !== 'object'
    ) {
      return [];
    }
    return Object.entries(expectedLabels).every(([key, value]) => session.labels[key] === value)
      ? [session.id]
      : [];
  });
}

export async function waitForContains(scope, options, testId, expected) {
  await poll(
    testId,
    async () =>
      await scope
        .getByTestId(testId)
        .textContent()
        .catch(() => ''),
    (value) => typeof value === 'string' && value.includes(expected),
    options.connectTimeoutMs,
  );
}

export async function assertNoHorizontalOverflow(page, testId, label) {
  const size = await page.getByTestId(testId).evaluate((element) => ({
    clientWidth: element.clientWidth,
    scrollWidth: element.scrollWidth,
  }));
  assertFits(size, label);
}

export async function assertNoBodyHorizontalOverflow(page, label) {
  const size = await page.evaluate(() => ({
    clientWidth: document.documentElement.clientWidth,
    scrollWidth: document.documentElement.scrollWidth,
  }));
  assertFits(size, label);
}

export function assertEqual(actual, expected, label) {
  if (actual !== expected) {
    throw new Error(
      `Expected ${label} to be ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`,
    );
  }
}

function assertFits(size, label) {
  if (size.scrollWidth > size.clientWidth + 1) {
    throw new Error(`${label} overflows horizontally: ${JSON.stringify(size)}`);
  }
}
