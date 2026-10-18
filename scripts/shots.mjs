// Takes screenshots of each screen from the built interface (browser preview data).
import { chromium } from 'playwright';
import { mkdirSync } from 'node:fs';

// A small valid PDF with text, built in memory, to try adding and reading a book.
function makePdf(pages) {
  const objs = [];
  const add = (body) => { objs.push(body); return objs.length; };
  add('<< /Type /Catalog /Pages 2 0 R >>');
  const kids = pages.map((_, i) => `${4 + i * 2} 0 R`).join(' ');
  add(`<< /Type /Pages /Kids [${kids}] /Count ${pages.length} >>`);
  add('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>');
  pages.forEach((lines, i) => {
    const content = 'BT /F1 22 Tf 50 740 Td 28 TL ' + lines.map((l) => `(${l}) Tj T*`).join(' ') + ' ET';
    add(`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents ${5 + i * 2} 0 R /Resources << /Font << /F1 3 0 R >> >> >>`);
    add(`<< /Length ${content.length} >>\nstream\n${content}\nendstream`);
  });
  let out = '%PDF-1.4\n';
  const offs = [];
  objs.forEach((b, i) => { offs.push(out.length); out += `${i + 1} 0 obj\n${b}\nendobj\n`; });
  const x = out.length;
  out += `xref\n0 ${objs.length + 1}\n0000000000 65535 f \n` + offs.map((o) => String(o).padStart(10, '0') + ' 00000 n \n').join('');
  out += `trailer\n<< /Size ${objs.length + 1} /Root 1 0 R >>\nstartxref\n${x}\n%%EOF`;
  return Buffer.from(out, 'latin1');
}

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
  await shoot(page, `${label}-1b-shelf`);
  await page.locator('#add-book').setInputFiles({
    name: 'where_there_is_no_doctor.pdf', mimeType: 'application/pdf',
    buffer: makePdf([['A book I added myself', 'Page one of the sample'], ['Cool a burn with clean running water', 'Page two of the sample'], ['Page three of the sample']]),
  });
  await page.getByText(/Added: Where There Is No Doctor/).waitFor();
  await shoot(page, `${label}-1c-shelf-added`);
  await page.getByRole('button', { name: /Open Where There Is No Doctor/ }).click();
  await page.waitForTimeout(1200);
  await shoot(page, `${label}-1d-reader`);
  await page.getByRole('button', { name: 'Next ›' }).click();
  await page.getByRole('button', { name: 'Search', exact: true }).click();
  await page.getByLabel('Search in this book').fill('burn');
  await page.getByRole('button', { name: 'Find' }).click();
  await page.waitForTimeout(800);
  await shoot(page, `${label}-1e-reader-search`);
  await page.getByRole('button', { name: '‹ Shelf' }).click();
  await page.getByRole('button', { name: 'Archives', exact: true }).click();
  await shoot(page, `${label}-1f-archives`);
  await page.getByRole('button', { name: 'Emergency cards', exact: true }).click();
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

  await page.getByRole('button', { name: 'Drills', exact: true }).click();
  await shoot(page, `${label}-9a-drills`);
  await page.getByRole('button', { name: 'Do it again' }).first().click();
  await page.locator('.steps input').nth(0).check();
  await page.locator('.steps input').nth(1).check();
  await page.locator('.steps input').nth(2).check();
  await shoot(page, `${label}-9b-drill-running`);
  await page.getByRole('button', { name: 'End drill without saving' }).click();

  await page.getByRole('button', { name: 'Binder', exact: true }).click();
  await shoot(page, `${label}-9c-binder`);

  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await shoot(page, `${label}-9d-settings`);
  await page.getByRole('tab', { name: 'Sources and licenses' }).click();
  await shoot(page, `${label}-9e-sources`);

  if (errors.length) console.log(label, 'console errors:', errors);
  await page.close();
}
await browser.close();
console.log('screenshots:', out.join(', '));
