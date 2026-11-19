import { chromium } from 'playwright-core';

// Set CHROME_PATH to a Chromium/Chrome executable, or leave it unset to use Playwright's own build.
const exe = process.env.CHROME_PATH;
const browser = await chromium.launch({ headless: true, ...(exe ? { executablePath: exe } : {}) });
const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
const errors = [];
page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));
page.on('console', (m) => { if (m.type() === 'error') errors.push(`console: ${m.text()}`); });

await page.goto('http://127.0.0.1:8123/index.html');
await page.waitForFunction(() => window.__last && window.__last !== '{}', null, { timeout: 30000 });
const now = () => page.evaluate(() => JSON.parse(window.__last));
const settle = () => new Promise((r) => setTimeout(r, 150));

const results = {};
results.initial = await now();

const canvas = page.locator('canvas');
await canvas.waitFor();
const box = await canvas.boundingBox();
console.log('canvas box', box);

// Pointer: move in, click (focuses the canvas), move around, wheel.
await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
await page.mouse.down();
await page.mouse.up();
await settle();
results.afterClick = await now();

await page.mouse.move(box.x + 20, box.y + 20, { steps: 12 });
await settle();
await page.mouse.wheel(0, 120);
await settle();
results.afterPointer = await now();

// Keyboard: one key, then a short string (text should arrive as IME commits).
await page.keyboard.press('a');
await settle();
results.afterKey = await now();
await page.keyboard.type('bc');
await settle();
results.afterType = await now();

// Resize: grow the canvas in CSS pixels, which the backend's ResizeObserver should see.
await page.evaluate(() => {
  const c = document.querySelector('canvas');
  c.style.width = '640px';
  c.style.height = '360px';
});
await page.waitForTimeout(400);
results.afterResize = await now();

// Shrink again, to check that a second resize is also reported.
await page.evaluate(() => {
  const c = document.querySelector('canvas');
  c.style.width = '320px';
  c.style.height = '200px';
});
await page.waitForTimeout(400);
results.afterShrink = await now();

for (const [k, v] of Object.entries(results)) console.log(k.padEnd(14), JSON.stringify(v));
console.log('errors:', errors.length ? errors : 'none');
await browser.close();
