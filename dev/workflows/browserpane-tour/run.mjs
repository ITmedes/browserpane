const JOURNEY = [
  { label: 'home', url: 'https://browserpane.io/' },
  {
    label: 'remote-browser-isolation-vs-vdi',
    url: 'https://browserpane.io/blog/remote-browser-isolation-vs-vdi.html',
    hrefFragment: 'remote-browser-isolation-vs-vdi',
  },
  { label: 'github', url: 'https://github.com/ITmedes/browserpane' },
];

export default async function run({ page, input }) {
  const options = parseOptions(input);
  const visited = [];
  await openPage(page, JOURNEY[0].url);
  visited.push(await slowScrollPage(page, JOURNEY[0].label, options));
  await clickLinkOrNavigate(page, JOURNEY[1]);
  visited.push(await slowScrollPage(page, JOURNEY[1].label, options));
  await openPage(page, JOURNEY[2].url);
  visited.push(await slowScrollPage(page, JOURNEY[2].label, options));
  return { visited, final_url: page.url() };
}

function parseOptions(input) {
  const value = input && typeof input === 'object' ? input : {};
  return {
    delayMs: numberOption(value.scroll_delay_ms, 180, 50, 1000),
    stepPx: numberOption(value.scroll_step_px, 260, 80, 900),
    maxSteps: numberOption(value.max_scroll_steps, 180, 20, 400),
  };
}

function numberOption(value, fallback, min, max) {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return fallback;
  }
  return Math.min(max, Math.max(min, Math.round(value)));
}

async function openPage(page, url) {
  console.log(`opening ${url}`);
  await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 60000 });
  await settlePage(page);
}

async function clickLinkOrNavigate(page, target) {
  const link = await firstVisibleLink(page, target.hrefFragment);
  if (link) {
    console.log(`clicking ${target.url}`);
    await link.scrollIntoViewIfNeeded({ timeout: 5000 }).catch(() => {});
    await Promise.all([
      page.waitForURL((url) => url.href.includes(target.hrefFragment), { timeout: 30000 }),
      link.click({ timeout: 10000 }),
    ]);
    await settlePage(page);
    return;
  }
  await openPage(page, target.url);
}

async function firstVisibleLink(page, hrefFragment) {
  const links = page.locator(`a[href*="${hrefFragment}"]`);
  const count = await links.count();
  for (let index = 0; index < count; index += 1) {
    const candidate = links.nth(index);
    if (await candidate.isVisible().catch(() => false)) {
      return candidate;
    }
  }
  return null;
}

async function settlePage(page) {
  await page.waitForLoadState('networkidle', { timeout: 15000 }).catch(() => {});
  await page.waitForTimeout(800);
}

async function slowScrollPage(page, label, options) {
  console.log(`scrolling ${label}`);
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.waitForTimeout(options.delayMs);
  let lastY = -1;
  for (let step = 0; step < options.maxSteps; step += 1) {
    const state = await page.evaluate((stepPx) => {
      const body = document.body;
      const root = document.documentElement;
      const maxY = Math.max(body.scrollHeight, root.scrollHeight) - window.innerHeight;
      window.scrollBy({ top: stepPx, left: 0, behavior: 'smooth' });
      return { maxY: Math.max(0, maxY), y: window.scrollY };
    }, options.stepPx);
    await page.waitForTimeout(options.delayMs);
    const currentY = await page.evaluate(() => window.scrollY);
    if (state.maxY <= 0 || currentY >= state.maxY - 2 || currentY === lastY) {
      return { label, url: page.url(), scroll_y: currentY, max_scroll_y: state.maxY };
    }
    lastY = currentY;
  }
  const finalY = await page.evaluate(() => window.scrollY);
  return { label, url: page.url(), scroll_y: finalY, max_scroll_y: finalY };
}
