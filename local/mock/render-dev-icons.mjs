// Renders the PNGs of the icons for instances with `instance_name` (assets/icons/dev) from their
// SVGs, like the ones in assets/icons: run from local/mock after changing dev/icon.svg or
// dev/favicon.svg.
//
//   node render-dev-icons.mjs
import { chromium } from 'playwright';
import fs from 'node:fs';
import path from 'node:path';

const dir = path.resolve(import.meta.dirname, '../../cable-editor-frontend/assets/icons/dev');
const icons = [
  ['icon.svg', 'icon-192.png', 192],
  ['icon.svg', 'icon-512.png', 512],
  ['icon.svg', 'apple-touch-icon.png', 180],
  ['favicon.svg', 'favicon-32.png', 32],
];

const browser = await chromium.launch();
for (const [svg, png, size] of icons) {
  const page = await browser.newPage({ viewport: { width: size, height: size } });
  const markup = fs.readFileSync(path.join(dir, svg), 'utf8');
  await page.setContent(
    `<style>html,body{margin:0;background:transparent}svg{display:block;width:${size}px;height:${size}px}</style>${markup}`,
  );
  await page.screenshot({ path: path.join(dir, png), omitBackground: true });
  await page.close();
  console.log(png);
}
await browser.close();
