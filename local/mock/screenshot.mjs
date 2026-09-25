// Screenshots of frontend pages served by server.mjs, as on a phone (Pixel 7) or with --desktop.
//
//   node screenshot.mjs [--desktop] [--full] [--out DIR] /plan/0/cabinet/1/overview ...
//
// Logs in through the mock OIDC provider first. Web Bluetooth is stubbed, so the label printing
// controls show up. For each page it prints the layout width (wider than the device means
// something overflows) and any console errors.
import { chromium, devices } from 'playwright';
import fs from 'node:fs';

const args = process.argv.slice(2);
const flag = (name) => args.includes(name) && args.splice(args.indexOf(name), 1).length > 0;
const desktop = flag('--desktop');
const fullPage = flag('--full');
const outIndex = args.indexOf('--out');
const out = outIndex >= 0 ? args.splice(outIndex, 2)[1] : 'screenshots';
const origin = `http://localhost:${process.env.MOCK_PORT ?? 8099}`;
fs.mkdirSync(out, { recursive: true });

const browser = await chromium.launch();
const context = await browser.newContext(desktop ? { viewport: { width: 1440, height: 900 } } : devices['Pixel 7']);
await context.addInitScript(() => {
  if (!navigator.bluetooth) {
    Object.defineProperty(navigator, 'bluetooth', { value: { getAvailability: async () => true } });
  }
});
const page = await context.newPage();
page.on('console', (m) => m.type() === 'error' && console.log('  console error:', m.text()));
page.on('pageerror', (e) => console.log('  page error:', e.message));

await page.goto(origin);
await page.waitForLoadState('networkidle');
for (const url of args.length ? args : ['/']) {
  await page.goto(origin + url);
  await page.waitForLoadState('networkidle');
  await page.waitForTimeout(1000);
  const file = `${out}/${(url.replace(/\W+/g, '_').replace(/^_|_$/g, '') || 'index')}${desktop ? '-desktop' : ''}.png`;
  await page.screenshot({ path: file, fullPage });
  const width = await page.evaluate(() => window.innerWidth);
  const device = page.viewportSize().width;
  console.log(`${url} -> ${file}, layout width ${width}px${width > device ? ` (device ${device}px: overflow!)` : ''}`);
}
await browser.close();
