/**
 * E2E test: Load the BrowserPane JS bundle, create an InputController with mocked deps,
 * and verify Cmd+Shift+Arrow produces the correct protocol messages (Home/End with Shift).
 */
import { chromium } from 'playwright';

const WEB_URL = 'http://localhost:8080';

(async () => {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36',
  });
  const page = await context.newPage();

  // Collect console messages
  const consoleLogs = [];
  page.on('console', (msg) => {
    consoleLogs.push(msg.text());
  });

  // Load a minimal page that imports the BrowserPane bundle
  await page.goto(WEB_URL, { waitUntil: 'domcontentloaded', timeout: 10000 });

  // Inject a test harness: import the SDK, create InputController with mocked deps
  const result = await page.evaluate(async () => {
    const mod = await import('/dist/bpane.js');

    // Find the InputController export - it may be named differently in the bundle
    // Let's check what's exported
    const exports = Object.keys(mod);

    // We need to access InputController. It might not be directly exported from bpane.ts.
    // Let's return the exports so we know what we're working with.
    return { exports };
  });

  console.log('SDK exports:', result.exports);

  // Since InputController may not be directly exported, let's test at a lower level.
  // Create a canvas, add event listeners manually, and observe what the BrowserPane code does.
  const testResult = await page.evaluate(async () => {
    // Create a test canvas
    const canvas = document.createElement('canvas');
    canvas.width = 800;
    canvas.height = 600;
    canvas.tabIndex = 0;
    document.body.appendChild(canvas);
    canvas.focus();

    // Track what events the canvas receives
    const receivedEvents = [];
    canvas.addEventListener('keydown', (e) => {
      receivedEvents.push({
        type: 'keydown',
        code: e.code,
        key: e.key,
        metaKey: e.metaKey,
        shiftKey: e.shiftKey,
        ctrlKey: e.ctrlKey,
        altKey: e.altKey,
      });
    });
    canvas.addEventListener('keyup', (e) => {
      receivedEvents.push({
        type: 'keyup',
        code: e.code,
        key: e.key,
        metaKey: e.metaKey,
        shiftKey: e.shiftKey,
        ctrlKey: e.ctrlKey,
      });
    });

    // Simulate Cmd+Shift+ArrowLeft by dispatching synthetic KeyboardEvents
    // This is what happens when the user presses these keys on Mac
    canvas.dispatchEvent(new KeyboardEvent('keydown', {
      code: 'MetaLeft', key: 'Meta', metaKey: true, bubbles: true,
    }));
    canvas.dispatchEvent(new KeyboardEvent('keydown', {
      code: 'ShiftLeft', key: 'Shift', shiftKey: true, metaKey: true, bubbles: true,
    }));
    canvas.dispatchEvent(new KeyboardEvent('keydown', {
      code: 'ArrowLeft', key: 'ArrowLeft', shiftKey: true, metaKey: true, bubbles: true,
    }));
    canvas.dispatchEvent(new KeyboardEvent('keyup', {
      code: 'ArrowLeft', key: 'ArrowLeft', shiftKey: true, metaKey: true, bubbles: true,
    }));
    canvas.dispatchEvent(new KeyboardEvent('keyup', {
      code: 'ShiftLeft', key: 'Shift', metaKey: true, bubbles: true,
    }));
    canvas.dispatchEvent(new KeyboardEvent('keyup', {
      code: 'MetaLeft', key: 'Meta', bubbles: true,
    }));

    return { receivedEvents };
  });

  console.log('\nRaw KeyboardEvents received by canvas (synthetic dispatch):');
  for (const ev of testResult.receivedEvents) {
    console.log(`  ${ev.type}: code=${ev.code} key=${ev.key} meta=${ev.metaKey} shift=${ev.shiftKey} ctrl=${ev.ctrlKey}`);
  }

  // Now the real test: use Playwright's keyboard API which generates "real" browser events
  console.log('\n--- Testing with Playwright keyboard API (real browser events) ---');

  const realResult = await page.evaluate(() => {
    const canvas = document.querySelector('canvas');
    if (!canvas) return { error: 'no canvas' };
    window.__realEvents = [];
    canvas.addEventListener('keydown', (e) => {
      window.__realEvents.push({
        type: 'keydown', code: e.code, key: e.key,
        metaKey: e.metaKey, shiftKey: e.shiftKey, ctrlKey: e.ctrlKey,
      });
    }, { capture: true });
    canvas.addEventListener('keyup', (e) => {
      window.__realEvents.push({
        type: 'keyup', code: e.code, key: e.key,
        metaKey: e.metaKey, shiftKey: e.shiftKey, ctrlKey: e.ctrlKey,
      });
    }, { capture: true });
    canvas.focus();
    return { ok: true };
  });

  // Focus canvas via click
  await page.click('canvas');
  await page.waitForTimeout(200);

  // Playwright keyboard: Meta+Shift+ArrowLeft
  await page.keyboard.down('Meta');
  await page.keyboard.down('Shift');
  await page.keyboard.down('ArrowLeft');
  await page.waitForTimeout(50);
  await page.keyboard.up('ArrowLeft');
  await page.keyboard.up('Shift');
  await page.keyboard.up('Meta');
  await page.waitForTimeout(200);

  const realEvents = await page.evaluate(() => window.__realEvents);
  console.log('\nReal browser keydown/keyup events on canvas:');
  if (realEvents && realEvents.length > 0) {
    for (const ev of realEvents) {
      console.log(`  ${ev.type}: code=${ev.code} key=${ev.key} meta=${ev.metaKey} shift=${ev.shiftKey} ctrl=${ev.ctrlKey}`);
    }
  } else {
    console.log('  (NO events received — Playwright keyboard events do not reach canvas!)');
  }

  // Now test: import the BrowserPane module and create InputController with mocked sendFrame
  console.log('\n--- Testing InputController directly ---');
  const icTest = await page.evaluate(async () => {
    const mod = await import('/dist/bpane.js');

    // The module might export InputController or it might be internal.
    // Let's check for common export patterns
    const ic = mod.InputController;
    if (!ic) {
      return { error: 'InputController not exported', exports: Object.keys(mod) };
    }

    const canvas2 = document.createElement('canvas');
    canvas2.width = 800;
    canvas2.height = 600;
    canvas2.tabIndex = 0;
    document.body.appendChild(canvas2);
    canvas2.focus();

    const sentFrames = [];
    const controller = new ic({
      canvas: canvas2,
      sendFrame: (channelId, payload) => {
        sentFrames.push({ channelId, payload: Array.from(payload) });
      },
      drawCursor: () => {},
      getRemoteDims: () => ({ width: 800, height: 600 }),
      clipboardEnabled: false,
    });
    controller.serverSupportsKeyEventEx = true;
    controller.setup();

    // Dispatch Cmd+Shift+ArrowLeft
    canvas2.dispatchEvent(new KeyboardEvent('keydown', {
      code: 'MetaLeft', key: 'Meta', metaKey: true, bubbles: true,
    }));
    canvas2.dispatchEvent(new KeyboardEvent('keydown', {
      code: 'ShiftLeft', key: 'Shift', shiftKey: true, metaKey: true, bubbles: true,
    }));
    canvas2.dispatchEvent(new KeyboardEvent('keydown', {
      code: 'ArrowLeft', key: 'ArrowLeft', shiftKey: true, metaKey: true, bubbles: true,
    }));

    // Small delay simulation
    await new Promise(r => setTimeout(r, 50));

    canvas2.dispatchEvent(new KeyboardEvent('keyup', {
      code: 'ArrowLeft', key: 'ArrowLeft', shiftKey: true, metaKey: true, bubbles: true,
    }));
    canvas2.dispatchEvent(new KeyboardEvent('keyup', {
      code: 'ShiftLeft', key: 'Shift', metaKey: true, bubbles: true,
    }));
    canvas2.dispatchEvent(new KeyboardEvent('keyup', {
      code: 'MetaLeft', key: 'Meta', bubbles: true,
    }));

    await new Promise(r => setTimeout(r, 50));

    controller.destroy();
    return { sentFrames, frameCount: sentFrames.length };
  });

  if (icTest.error) {
    console.log('Error:', icTest.error);
    if (icTest.exports) console.log('Available exports:', icTest.exports);
  } else {
    console.log(`InputController sent ${icTest.frameCount} frames:`);
    // Parse the frames: CH_INPUT = 2
    for (const frame of icTest.sentFrames) {
      const p = frame.payload;
      const msgType = p[0];
      if (msgType === 0x05) { // INPUT_KEY_EVENT_EX
        const keycode = p[1] | (p[2] << 8) | (p[3] << 16) | (p[4] << 24);
        const down = p[5] === 1;
        const modifiers = p[6];
        const keyChar = p[7] | (p[8] << 8) | (p[9] << 16) | (p[10] << 24);
        // Known evdev codes
        const names = { 42: 'ShiftLeft', 54: 'ShiftRight', 102: 'Home', 107: 'End', 125: 'MetaLeft', 126: 'MetaRight' };
        const name = names[keycode] || `evdev:${keycode}`;
        const modNames = [];
        if (modifiers & 0x01) modNames.push('CTRL');
        if (modifiers & 0x02) modNames.push('ALT');
        if (modifiers & 0x04) modNames.push('SHIFT');
        if (modifiers & 0x08) modNames.push('META');
        if (modifiers & 0x10) modNames.push('ALTGR');
        console.log(`  KeyEventEx: ${name}(${keycode}) ${down ? 'DOWN' : 'UP'} mods=[${modNames.join('|')}] char=${keyChar}`);
      } else if (msgType === 0x04) { // INPUT_KEY_EVENT
        const keycode = p[1] | (p[2] << 8) | (p[3] << 16) | (p[4] << 24);
        const down = p[5] === 1;
        const modifiers = p[6];
        const names = { 42: 'ShiftLeft', 102: 'Home', 107: 'End', 125: 'MetaLeft' };
        const name = names[keycode] || `evdev:${keycode}`;
        console.log(`  KeyEvent: ${name}(${keycode}) ${down ? 'DOWN' : 'UP'} mods=0x${modifiers.toString(16)}`);
      } else {
        console.log(`  MsgType=0x${msgType.toString(16)} payload=[${p.slice(0, 12).join(',')}...]`);
      }
    }
  }

  // Print console logs
  console.log('\n--- Browser console logs ---');
  consoleLogs.forEach(l => console.log(`  ${l}`));

  await browser.close();
  console.log('\nDone.');
})().catch((err) => {
  console.error('Test failed:', err.message);
  process.exit(1);
});
