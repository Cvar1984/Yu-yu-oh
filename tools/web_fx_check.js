// Plays duels in the browser game with animations, effects and music on,
// and checks: every common cue is animated and sounded, the music runs, a
// tap skips the animations, the Animations switch turns them off, an
// owner's sound file (if the build has audio/summon.*) is loaded in place
// of the built-in sound, and the page logs no errors. Screenshots of an
// attack and a banner go to the screenshot directory.
//
// Usage (with the page served, e.g. python3 -m http.server 8765 -d web/play/dist):
//   node tools/web_fx_check.js [games] [url] [screenshot-dir]
// IMAGE_DIR=<dir with small/ and full/> answers the YGOPRODeck image URLs
// from a local folder.
const { chromium } = require('playwright');
const fs = require('fs'), path = require('path');
const games = Number(process.argv[2] || 2);
const url = process.argv[3] || 'http://127.0.0.1:8765/';
const dir = process.argv[4] || '.';

function assert(cond, what) { if (!cond) throw new Error(what); }

(async () => {
  const browser = await chromium.launch({ args: ['--autoplay-policy=no-user-gesture-required'] });
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  await page.route('https://images.ygoprodeck.com/**', (route) => {
    const imageDir = process.env.IMAGE_DIR;
    const u = new URL(route.request().url());
    const file = imageDir && path.join(imageDir, u.pathname.includes('cards_small') ? 'small' : 'full', path.basename(u.pathname));
    return file && fs.existsSync(file)
      ? route.fulfill({ status: 200, contentType: 'image/jpeg', body: fs.readFileSync(file) })
      : route.fulfill({ status: 404 });
  });
  const idle = () => page.waitForFunction(() => !state.busy, null, { timeout: 60000 });
  const shots = { attack: false, banner: false };
  let biggest = 0, moves = 0;

  for (let g = 0; g < games; g++) {
    await page.goto(url);
    await page.evaluate(() => localStorage.clear());
    await page.reload();
    await page.waitForSelector('#bot');
    // The biggest answer the engine sends (steps included).
    await page.evaluate(() => {
      const call = Engine.call.bind(Engine);
      window.__biggest = 0;
      Engine.call = (cmd) => { const a = call(cmd); window.__biggest = Math.max(window.__biggest, JSON.stringify(a).length); return a; };
    });
    await page.selectOption('#bot', 'network');
    await page.click('button[type=submit]');   // a real tap: sound starts
    await page.waitForSelector('.bar');
    let rng = 11 + g;
    const rand = (n) => { rng = (rng * 1103515245 + 12345) % 2147483648; return rng % n; };
    for (let step = 0; step < 250; step++) {
      await idle();
      const v = await page.evaluate(() => ({ n: state.view.actions.length, over: state.view.result !== null, kinds: state.view.actions.map((a) => a.kind) }));
      if (v.over || !v.n) break;
      // Prefer moves that make something happen, so the duel moves on.
      const lively = v.kinds.map((k, i) => [k, i]).filter(([k]) => ['summon', 'attack', 'activate', 'battle', 'set'].includes(k));
      const i = lively.length && rand(3) ? lively[rand(lively.length)][1] : rand(v.n);
      await page.evaluate((i) => play(state.view.actions[i].index), i);
      moves++;
      for (let k = 0; k < 12 && !(shots.attack && shots.banner); k++) {
        const seen = await page.evaluate(() => ({ busy: state.busy, burst: !!document.querySelector('#fx .burst.hit'), banner: !!document.querySelector('#fx .banner') }));
        if (!seen.busy) break;
        if (seen.burst && !shots.attack) { await page.screenshot({ path: `${dir}/fx-attack.png` }); shots.attack = true; }
        if (seen.banner && !shots.banner) { await page.screenshot({ path: `${dir}/fx-banner.png` }); shots.banner = true; }
        await page.waitForTimeout(60);
      }
    }
    await idle();
    biggest = Math.max(biggest, await page.evaluate(() => window.__biggest));
    console.log(`game ${g + 1}:`, await page.evaluate(() => state.view.result ?? 'turn ' + state.view.turn));
  }

  const r = await page.evaluate(() => ({
    fx: [...new Set(FX.log)].sort(), sounds: [...new Set(Sound.played)].sort(),
    ctx: Sound.ctx?.state, music: Music.timer !== null || Music.source !== null,
    files: Object.keys(Sound.files), buffers: Object.keys(Sound.buffers),
  }));
  console.log('animated:', r.fx.join(' '));
  console.log('sounds:', r.sounds.join(' '));
  for (const k of ['turn', 'draw', 'summon', 'set', 'activate', 'attack', 'damage', 'destroy']) assert(r.fx.includes(k), `no ${k} animation`);
  for (const k of ['turn', 'draw', 'summon', 'set', 'activate', 'attack', 'hit', 'damage', 'destroy']) assert(r.sounds.includes(k), `no ${k} sound`);
  assert(r.ctx === 'running', `audio is ${r.ctx}`);
  assert(r.music, 'the music is not playing');
  if (r.files.includes('summon')) {
    await page.waitForFunction(() => 'summon' in Sound.buffers, null, { timeout: 5000 });
    console.log('own sound files loaded:', r.files.join(' '));
  }

  // A tap skips the animations; with Animations off a move shows at once.
  await page.goto(url);
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await page.waitForSelector('#bot');
  await page.selectOption('#bot', 'random');
  await page.click('[data-first="bot"]');
  await page.click('button[type=submit]');
  const busyAtStart = await page.evaluate(() => state.busy);
  assert(busyAtStart, 'the bot\'s opening moves are animated');
  await page.mouse.click(640, 450);
  await page.waitForTimeout(700);
  assert(!(await page.evaluate(() => state.busy)), 'a tap skips the animations');
  await page.click('.av summary');
  await page.click('[data-av="motion"]');
  assert(await page.evaluate(() => av.motion === false && JSON.parse(localStorage.getItem('duel-table-av')).motion === false), 'Animations off is remembered');
  const instant = await page.evaluate(() => {
    const a = state.view.actions.find((x) => x.kind === 'end_turn') ?? state.view.actions[0];
    play(a.index);
    return !state.busy;
  });
  assert(instant, 'with Animations off a move shows at once');
  await page.screenshot({ path: `${dir}/fx-settings.png` });

  console.log(`moves: ${moves}; biggest answer: ${(biggest / 1024).toFixed(0)} KB; screenshots: ${JSON.stringify(shots)}`);
  console.log('errors', JSON.stringify(errors));
  await browser.close();
  if (errors.length) process.exit(1);
})().catch((e) => { console.error('FAIL', e.message); process.exit(1); });
