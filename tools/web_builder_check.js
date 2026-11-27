// Drives the browser deck builder through its controls only and checks it
// against data/cards.json read here, independently of the page: filter
// counts, copy limits, filling and swapping slots, saving, the saved deck in
// the duel setup and a duel started with it, export and import. Fails on
// any page error.
//
// Usage (with the page served, e.g. python3 -m http.server 8765 -d web/play/dist):
//   node tools/web_builder_check.js [url] [screenshot-dir]
// IMAGE_DIR=<dir with small/ and full/> answers the YGOPRODeck image URLs
// from a local folder (none are fetched otherwise; cards then show as text).
// Writes the exported deck to <screenshot-dir>/exported.json, so it can be
// checked with `ygo check-deck`.
const { chromium } = require('playwright');
const fs = require('fs'), path = require('path');
const url = process.argv[2] || 'http://127.0.0.1:8765/';
const dir = process.argv[3] || '.';
const root = path.join(__dirname, '..');
const cards = JSON.parse(fs.readFileSync(path.join(root, 'data/cards.json'), 'utf8')).cards;
const banlist = JSON.parse(fs.readFileSync(path.join(root, 'data/banlist.json'), 'utf8'));
const limitOf = (id) => banlist.limits.find((l) => l.id === id)?.limit ?? banlist.default_limit;

function assert(cond, what) { if (!cond) throw new Error(what); }

(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, acceptDownloads: true });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  page.on('dialog', (d) => d.accept());
  await page.route('https://images.ygoprodeck.com/**', (route) => {
    const imageDir = process.env.IMAGE_DIR;
    const u = new URL(route.request().url());
    const file = imageDir && path.join(imageDir, u.pathname.includes('cards_small') ? 'small' : 'full', path.basename(u.pathname));
    return file && fs.existsSync(file)
      ? route.fulfill({ status: 200, contentType: 'image/jpeg', body: fs.readFileSync(file) })
      : route.fulfill({ status: 404 });
  });

  await page.goto(url);
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await page.click('#open-builder');
  const found = () => page.textContent('#b-found');
  assert(await found() === `${cards.length} of ${cards.length}`, `whole pool shown: ${await found()}`);

  // Filters, against counts computed here.
  const expect = async (setFilters, pred, what) => {
    await page.click('#b-clear');
    await setFilters();
    const want = cards.filter(pred).length;
    const got = await found();
    assert(got === `${want} of ${cards.length}`, `${what}: page shows ${got}, data has ${want}`);
    console.log(`filter ${what}: ${want}`);
  };
  await expect(() => page.selectOption('[name=kind]', 'trap'), (c) => c.type === 'Trap Card', 'Traps');
  await expect(() => page.selectOption('[name=set]', 'PSV'), (c) => c.in_game_index === 'PSV', 'set PSV');
  await expect(async () => { await page.selectOption('[name=attr]', 'DARK'); await page.fill('[name=atk]', '2000'); },
    (c) => c.attribute === 'DARK' && c.atk >= 2000, 'DARK, ATK 2000 or more');
  await expect(async () => { await page.selectOption('[name=lmin]', '5'); await page.selectOption('[name=lmax]', '6'); await page.selectOption('[name=race]', 'Dragon'); },
    (c) => c.race === 'Dragon' && c.level >= 5 && c.level <= 6, 'Dragons of Level 5 to 6');
  await expect(() => page.fill('[name=q]', 'dragon'),
    (c) => c.name.toLowerCase().includes('dragon') || c.text.toLowerCase().includes('dragon'), 'search "dragon"');
  const first = await page.getAttribute('#b-results .btile:first-child [data-pick]', 'title');
  assert(first.toLowerCase().includes('dragon'), `name matches first: ${first}`);

  // A limited card: the + button stops at its limit.
  const pot = cards.find((c) => c.name === 'Pot of Greed');
  await page.click('#b-clear');
  await page.fill('[name=q]', 'Pot of Greed');
  await page.click(`[data-add="${pot.id}"]`);
  assert(await page.isDisabled(`.btile [data-add="${pot.id}"]`), 'Pot of Greed: + disabled at its limit of 1');
  assert(limitOf(pot.id) === 1, 'the ban list limits Pot of Greed to 1');

  // Fill the Main Deck to 40 with Normal Monsters of Level 4 or less, at
  // their copy limits, by tapping +.
  await page.click('#b-clear');
  await page.selectOption('[name=kind]', 'normal');
  await page.selectOption('[name=lmax]', '4');
  await page.selectOption('[name=sort]', 'atk');
  let added = 1;
  for (const id of await page.$$eval('#b-results [data-add]', (bs) => bs.map((b) => Number(b.dataset.add)))) {
    for (let k = 0; k < limitOf(id) && added < 40; k++, added++) await page.click(`.btile [data-add="${id}"]`);
    if (added >= 40) break;
  }
  const status = await page.textContent('#b-status');
  assert(status.includes('Legal'), `40 cards are legal: ${status}`);
  assert((await page.textContent('#b-main-count')).startsWith('40 '), 'Main Deck count 40');
  await page.screenshot({ path: `${dir}/builder-desk.png` });

  // Swap slot 1 (Pot of Greed) for Raigeki.
  await page.click('[data-slot="main:0"]');
  await page.click('#b-clear');
  await page.fill('[name=q]', 'Raigeki');
  const raigeki = cards.find((c) => c.name === 'Raigeki');
  await page.click(`[data-pick="${raigeki.id}"]`);
  const slot1 = await page.getAttribute('[data-slot="main:0"]', 'title');
  assert(slot1 === 'Raigeki', `slot 1 now holds ${slot1}`);
  assert((await page.textContent('#b-status')).includes('swapped'), 'the swap is reported');
  // A Fusion cannot go in a Main Deck slot.
  await page.click('[data-slot="main:1"]');
  await page.click('#b-clear');
  await page.selectOption('[name=kind]', 'fusion');
  await page.click('#b-results [data-pick]');
  assert((await page.textContent('#b-status')).includes('goes in the Extra Deck'), 'a Fusion is refused in a Main Deck slot');
  await page.keyboard.press('Escape');

  // Save, then find it in the duel setup and start a duel with it.
  await page.fill('#b-name', 'Builder check deck');
  await page.click('#b-save');
  assert((await page.textContent('#b-status')).includes('Saved'), 'saved');
  const download = page.waitForEvent('download');
  await page.click('#b-export');
  const file = await (await download).path();
  fs.copyFileSync(file, `${dir}/exported.json`);
  const exported = JSON.parse(fs.readFileSync(file, 'utf8'));
  const total = exported.main.reduce((n, e) => n + e.count, 0);
  assert(exported.name === 'Builder check deck' && total === 40, `exported ${exported.name}, ${total} cards`);
  assert(exported.main.some((e) => e.id === raigeki.id) && !exported.main.some((e) => e.id === pot.id), 'export has the swap');
  await page.click('#b-back');
  const mine = await page.$$eval('#deck optgroup[label="Your decks"] option', (os) => os.map((o) => o.textContent));
  assert(mine.includes('Builder check deck'), `in the setup's Your decks: ${mine}`);
  await page.selectOption('#deck', { label: 'Builder check deck' });
  await page.selectOption('#bot', 'random');
  await page.click('button[type=submit]');
  await page.waitForSelector('.bar');
  const started = await page.evaluate(() => state.game.deck);
  assert(started.startsWith('my-'), `duel started with the built deck (${started})`);
  // It survives a reload: the game resumes with the saved deck.
  await page.reload();
  await page.waitForSelector('.bar');
  console.log('duel with the built deck: started and resumed after a reload');

  // Import a shipped deck file.
  await page.evaluate(() => { state.game = null; state.view = null; save(); renderSetup(); });
  await page.click('#open-builder');
  await page.setInputFiles('#b-import', path.join(root, 'data/decks/enemy_e1_toon.json'));
  const imported = await page.inputValue('#b-name');
  assert((await page.textContent('#b-main-count')).startsWith('40 '), 'imported 40 cards');
  assert((await page.textContent('#b-status')).includes('Legal'), 'imported deck is legal');
  console.log('imported:', imported);

  // A phone.
  await page.setViewportSize({ width: 390, height: 844 });
  const wide = await page.evaluate(() => document.documentElement.scrollWidth);
  assert(wide <= 390, `no sideways scroll on a phone: ${wide}px`);
  await page.screenshot({ path: `${dir}/builder-phone.png`, fullPage: false });

  console.log('errors', JSON.stringify(errors));
  await browser.close();
  if (errors.length) process.exit(1);
})().catch((e) => { console.error('FAIL', e.message); process.exit(1); });
