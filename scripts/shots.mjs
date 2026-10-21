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
let failed = false;

async function shoot(page, name) {
  await page.waitForTimeout(250);
  await page.screenshot({ path: `shots/${name}.png`, fullPage: true });
  out.push(name);
}

// The loading screen (it only shows in a browser tab when the address ends with ?splash).
{
  const page = await browser.newPage({ viewport: { width: 1180, height: 780 } });
  await page.goto(url + '?splash');
  await page.getByText('Loading...').waitFor();
  await page.waitForTimeout(400);
  await page.screenshot({ path: 'shots/splash.png' });
  out.push('splash');
  await page.waitForSelector('.splash', { state: 'detached', timeout: 10000 });
  await page.close();
}

for (const [label, size] of [['desktop', { width: 1180, height: 780 }], ['phone', { width: 390, height: 800 }]]) {
  const page = await browser.newPage({ viewport: size });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  await page.goto(url);
  try {
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
  // Ghostnet, the offline internet: the start page, the browser, bookmarks, tabs and a search across every archive.
  await page.getByRole('button', { name: 'Ghostnet', exact: true }).click();
  await page.getByRole('button', { name: /Sample library \(preview\)/ }).waitFor();
  await shoot(page, `${label}-1g-ghostnet-start`);
  await page.getByRole('button', { name: /Sample library \(preview\)/ }).click();
  const zim = page.frameLocator('iframe[title="Archive page"]');
  await zim.getByRole('heading', { name: 'Sample library' }).waitFor();
  await zim.getByRole('link', { name: 'Water' }).click();
  await zim.getByText('Boil water for one minute').waitFor();
  await page.getByRole('button', { name: 'Bookmark this page' }).click();
  await page.getByRole('button', { name: 'Back', exact: true }).click();
  await zim.getByRole('heading', { name: 'Sample library' }).waitFor();
  await zim.getByRole('link', { name: 'An outside link' }).click();
  await page.getByText(/not opened/).waitFor();
  await page.getByLabel('Search every archive').fill('fir');
  await page.getByRole('option', { name: /Fire/ }).click();
  await zim.getByText('Keep fuel and tinder dry').waitFor();
  await page.getByRole('button', { name: 'New tab' }).click();
  await page.getByLabel('Search every archive').fill('water');
  await page.getByRole('button', { name: 'Search', exact: true }).click();
  await page.getByRole('heading', { name: /Results for/ }).waitFor();
  await shoot(page, `${label}-1h-ghostnet-search`);
  await page.getByRole('button', { name: /^Bookmarks/ }).click();
  await page.locator('.marks').getByRole('button', { name: /^Water/ }).waitFor();
  await shoot(page, `${label}-1i-ghostnet-bookmarks`);
  await page.getByRole('button', { name: /^Bookmarks/ }).click();
  await page.getByRole('button', { name: 'Library', exact: true }).click();
  await page.getByRole('button', { name: 'Emergency cards', exact: true }).click();
  await page.getByRole('button', { name: /Low blood sugar/ }).click();
  await shoot(page, `${label}-2-library`);

  await page.getByRole('button', { name: 'Maps', exact: true }).click();
  await shoot(page, `${label}-2b-maps-empty`);
  await page.getByLabel('Map name').fill('Home to the lake cabin');
  for (const [lab, lat, lon] of [['Home', '39.4', '-80.1'], ['Bridge', '39.45', '-80.05'], ['Cabin', '39.5', '-79.95']]) {
    await page.getByLabel('Place label').fill(lab);
    await page.getByLabel('Latitude', { exact: true }).fill(lat);
    await page.getByLabel('Longitude', { exact: true }).fill(lon);
    await page.getByRole('button', { name: 'Add', exact: true }).click();
  }
  for (const n of [0, 1, 2]) await page.getByRole('button', { name: 'add to route' }).nth(n).click();
  for (const [lat, lon] of [['39.43', '-80.07'], ['39.43', '-80.03'], ['39.47', '-80.03']]) {
    await page.getByLabel('Corner latitude').fill(lat);
    await page.getByLabel('Corner longitude').fill(lon);
    await page.getByRole('button', { name: 'Add corner' }).click();
  }
  await page.getByRole('button', { name: 'Finish area' }).click();
  await page.getByText(/in a straight line through 3 points/).waitFor();
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await page.getByText('Saved.').waitFor();
  await shoot(page, `${label}-2c-maps-plan`);
  await page.getByLabel('Latitude', { exact: true }).fill('95');
  await page.getByLabel('Longitude', { exact: true }).fill('0');
  await page.getByRole('button', { name: 'Add', exact: true }).click();
  await page.getByText(/latitude between -90 and 90/).waitFor();

  // Academy: open a lesson, read it, take the quiz, see the score saved.
  await page.getByRole('button', { name: 'Academy', exact: true }).click();
  await page.getByRole('heading', { name: 'Academy' }).waitFor();
  await shoot(page, `${label}-2c1-academy-list`);
  await page.getByRole('button', { name: /Water comes first/ }).click();
  await page.getByRole('heading', { name: 'Water comes first' }).waitFor();
  await page.getByRole('button', { name: 'Take the quiz' }).click();
  await page.getByRole('radio', { name: 'About three days' }).check();
  await page.getByRole('button', { name: 'Check answers' }).click();
  await page.getByText('1 of 1 correct').waitFor();
  await shoot(page, `${label}-2c2-academy-quiz`);
  await page.getByRole('button', { name: 'Save result' }).click();
  await page.getByText('Saved: 1 of 1.').waitFor();

  // Music: browse, play a generated tone, watch the clock advance, use the equalizer.
  await page.getByRole('button', { name: 'Music', exact: true }).click();
  await page.getByRole('button', { name: 'Play Dawn' }).waitFor();
  await shoot(page, `${label}-2d-music-library`);
  await page.getByRole('button', { name: 'Artists', exact: true }).click();
  await page.getByRole('button', { name: /Other Band/ }).click();
  await page.getByRole('button', { name: 'Play Lamp' }).waitFor();
  await page.getByRole('button', { name: 'Play Moth' }).click();
  await page.getByRole('button', { name: 'Pause' }).waitFor();
  await page.waitForFunction(() => /^0:(0[1-9]|[1-5]\d)/.test(document.querySelector('.mini .time')?.textContent ?? ''), null, { timeout: 15000 });
  await shoot(page, `${label}-2e-music-playing`);
  await page.getByRole('tab', { name: /^Queue/ }).click();
  await shoot(page, `${label}-2f-music-queue`);
  await page.getByRole('tab', { name: 'Equalizer' }).click();
  await page.getByLabel('1 kHz').fill('6');
  await page.getByLabel('62 Hz').fill('9');
  await page.getByRole('button', { name: 'Pause' }).waitFor();
  await shoot(page, `${label}-2g-music-eq`);
  await page.getByRole('button', { name: 'Pause' }).click();

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
  await page.getByRole('tab', { name: 'Assistant' }).click();
  await shoot(page, `${label}-9d2-settings-assistant`);
  // a setting changes what the program does: with file search off, Ask no longer says it looks in files and folders
  await page.getByLabel(/Files and folders on the drive/).uncheck();
  await page.getByRole('button', { name: 'Ask', exact: true }).click();
  await page.getByText(/It looks in the emergency cards/).waitFor();
  if ((await page.locator('main').innerText()).includes('files and folders')) throw new Error('turning off file search did not change what Ask says');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('tab', { name: 'Plan' }).click();
  await shoot(page, `${label}-9d3-settings-plan`);
  await page.getByRole('tab', { name: 'Sources and licenses' }).click();
  await shoot(page, `${label}-9e-sources`);

  } catch (e) {
    // GitHub shows these as annotations on the job, so a failure can be read without the full log
    const msg = String((e && e.message) || e).split('\n').filter(Boolean).slice(0, 5).join(' | ').slice(0, 700);
    console.log(`::error::screenshots (${label}): ${msg}`);
    try { await page.screenshot({ path: `shots/FAILED-${label}.png`, fullPage: true }); } catch { /* nothing more to do */ }
    failed = true;
  }
  if (errors.length) console.log(label, 'console errors:', errors);
  await page.close();
}
await browser.close();
console.log('screenshots:', out.join(', '));
if (failed) process.exit(1);
