// Plays whole games through the browser page only: at each decision a
// random legal move is chosen, then made by tapping cards, menu items,
// targets, the bar and dialogs as a person would. Fails if the page plays
// any other move or a move cannot be reached; reports page errors.
// Needs Node with Playwright and the built page served at URL:
//   scripts/build-web.sh && python3 -m http.server 8765 -d web/play/dist &
//   node tools/web_ui_check.js [games] [screenshot.png] [url]
// Failed image loads are not errors (offline or blocked, the card shows as
// text). IMAGE_DIR=<dir> answers YGOPRODeck image URLs with <dir>/small or
// <dir>/full/<id>.jpg, to test the online images without the network.
const { chromium } = require('playwright');
const games = Number(process.argv[2] || 2);
const url = process.argv[4] || 'http://127.0.0.1:8765/';
(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => {
    if (m.type() !== 'error') return;
    if (/\.(jpg|png)$/.test(m.location()?.url ?? '') || /^Failed to load resource/.test(m.text())) return;
    errors.push(m.text());
  });
  let imagesServed = 0;
  if (process.env.IMAGE_DIR) {
    const fs = require('fs'), path = require('path');
    await page.route('https://images.ygoprodeck.com/**', (route) => {
      const u = new URL(route.request().url());
      const size = u.pathname.includes('cards_small') ? 'small' : 'full';
      const file = path.join(process.env.IMAGE_DIR, size, path.basename(u.pathname));
      if (!fs.existsSync(file)) return route.fulfill({ status: 404 });
      imagesServed++;
      return route.fulfill({ status: 200, contentType: 'image/jpeg', body: fs.readFileSync(file) });
    });
  }
  const kinds = {};
  let total = 0;
  for (let g = 0; g < games; g++) {
    await page.goto(url);
    await page.evaluate(() => window.localStorage.clear());
    await page.reload();
    await page.waitForSelector('#bot', { timeout: 30000 });
    await page.selectOption('#bot', 'network');
    await page.click('button[type=submit]');
    await page.waitForSelector('.bar', { timeout: 60000 });
    // Record what the page plays.
    await page.evaluate(() => {
      window.__played = [];
      const orig = play;
      play = (i) => { window.__played.push(i); orig(i); };
    });
    let rng = 7 + g * 1000;
    const rand = (n) => { rng = (rng * 1103515245 + 12345) % 2147483648; return rng % n; };
    for (let step = 0; step < 400; step++) {
      const v = await page.evaluate(() => ({ actions: state.view.actions, over: state.view.result !== null }));
      if (v.over || !v.actions.length) break;
      const a = v.actions[rand(v.actions.length)];
      kinds[a.kind] = (kinds[a.kind] || 0) + 1;
      const before = await page.evaluate(() => window.__played.length);
      const did = await page.evaluate(async (a) => {
        const click = (sel) => { const el = document.querySelector(sel); if (!el) return false; el.click(); return true; };
        const showCard = (inst) => {
          // Open the pile that holds it, if it is in a Graveyard.
          if (!document.querySelector(`[data-inst="${inst}"]`)) {
            for (const [side, key] of [[state.view.you, 'you'], [state.view.opponent, 'opponent']]) {
              if (side.graveyard.concat(side.banished).some((c) => c.instance === inst)) click(`[data-pile="${key}"]`);
            }
          }
          return document.querySelector(`[data-inst="${inst}"]`);
        };
        if (a.kind === 'answer') return click(`.dialog [data-move="${a.index}"]`) ? 'dialog' : 'no answer button';
        if (a.card === null) return click(`.bar [data-move="${a.index}"]`) ? 'bar' : 'no bar button';
        if (a.kind === 'choose' && !cardOnTable(a.card)) return click(`.dialog [data-move="${a.index}"]`) ? 'dialog tile' : 'no tile';
        const el = showCard(a.card);
        if (!el) return 'card not shown: ' + a.card;
        if (!el.classList.contains('can')) return 'card does not glow: ' + a.text;
        el.click();
        if (click(`#menu [data-move="${a.index}"]`)) return 'menu';
        const items = menuItems(a.card);
        const i = items.findIndex((it) => it.aim && it.aim.includes(state.view.actions[a.index]) || it.aim?.some((m) => m.index === a.index));
        if (i < 0) return 'not in the menu: ' + a.text;
        if (!click(`#menu [data-aim="${i}"]`)) return 'no aim item';
        for (const t of a.targets) {
          if (click(`.dialog [data-target="${t}"]`)) continue;
          const te = showCard(t);
          if (!te) return 'target not shown: ' + t;
          if (!te.classList.contains('target')) return 'target does not glow: ' + t;
          te.click();
        }
        if (click(`#menu [data-move="${a.index}"]`)) return 'variant';
        return 'aimed';
      }, a);
      const after = await page.evaluate(() => window.__played);
      if (after.length !== before + 1 || after[after.length - 1] !== a.index) {
        throw new Error(`move ${a.index} (${a.kind}: ${a.text}) not played via the page: ${did}; played ${JSON.stringify(after.slice(before))}`);
      }
      total++;
    }
    const status = await page.evaluate(() => state.view.result ?? ('turn ' + state.view.turn));
    console.log(`game ${g + 1}: ${status}`);
  }
  await page.evaluate(() => { const c = document.querySelector('.card.can'); if (c) c.click(); });
  await page.screenshot({ path: process.argv[3] || 'shot.png' });
  console.log('moves made through the page:', total, JSON.stringify(kinds));
  if (process.env.IMAGE_DIR) console.log('online images served:', imagesServed);
  console.log('errors', JSON.stringify(errors));
  await browser.close();
})().catch((e) => { console.error('FAIL', e.message); process.exit(1); });
