// Takes screenshots of each screen from the built interface (browser preview data).
import { chromium } from 'playwright';
import { mkdirSync } from 'node:fs';

const url = process.env.URL ?? 'http://localhost:4173/';
mkdirSync('shots', { recursive: true });
const browser = await chromium.launch();
const out = [];

async function shoot(page, name) {
  await page.waitForTimeout(250);
  await page.screenshot({ path: `shots/${name}.png`, fullPage: true });
  out.push(name);
}

for (const [label, size] of [['desktop', { width: 1180, height: 780 }], ['phone', { width: 390, height: 800 }]]) {
  const page = await browser.newPage({ viewport: size });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  await page.goto(url);
  await shoot(page, `${label}-1-ask`);

  await page.getByRole('button', { name: 'Library', exact: true }).click();
  await page.getByRole('button', { name: /Low blood sugar/ }).click();
  await shoot(page, `${label}-2-library`);

  await page.getByRole('button', { name: 'Plan', exact: true }).click();
  await shoot(page, `${label}-3-start`);

  await page.getByRole('button', { name: 'Inventory', exact: true }).click();
  await page.getByLabel(/Select Test strips \(Person 2\)/).check();
  await page.getByLabel(/Select Drinking water, 1 gal jugs/).first().check();
  await shoot(page, `${label}-4-inventory-selected`);
  await page.getByRole('button', { name: 'Delete selected' }).click();
  await shoot(page, `${label}-5-inventory-confirm`);
  await page.getByRole('button', { name: 'Cancel' }).click();
  await page.getByRole('button', { name: 'Create new item' }).click();
  await shoot(page, `${label}-6-inventory-new`);
  await page.getByRole('button', { name: 'Cancel' }).click();

  await page.getByRole('button', { name: 'Family', exact: true }).click();
  await shoot(page, `${label}-7-family`);

  await page.getByRole('button', { name: 'What fails', exact: true }).click();
  await shoot(page, `${label}-8-what-fails`);

  if (errors.length) console.log(label, 'console errors:', errors);
  await page.close();
}
await browser.close();
console.log('screenshots:', out.join(', '));
