// Prend les captures de l'essai de relecture : le panneau monté seul, en clair et en sombre,
// dans quatre états, avec les mesures prises dans la page.
//
//   node .claude/tests/design-reviewer/capture/capture.cjs <dossier de sortie>
//
// À lancer une fois le changement copié dans src/lib/git/ (voir le README de l'essai).
// Outil ponctuel, hors des dépendances du projet : il lui faut Playwright et son Chromium.
const { spawn } = require('node:child_process');
const { mkdirSync, writeFileSync } = require('node:fs');
const { join, resolve } = require('node:path');
const { chromium } = require('playwright');

const root = resolve(__dirname, '../../../..');
const out = resolve(process.argv[2] ?? 'screens');
const PORT = 5199;
const PAGE = `http://localhost:${PORT}/.claude/tests/design-reviewer/capture/index.html`;

/** Attend que le serveur de développement réponde. */
async function ready() {
  for (let i = 0; i < 120; i++) {
    try {
      if ((await fetch(PAGE)).ok) return;
    } catch {}
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error('le serveur de développement ne répond pas');
}

/** Tabule jusqu'à l'élément voulu : le focus vient ainsi du clavier, et son anneau est visible. */
async function tabTo(page, selector) {
  for (let i = 0; i < 20; i++) {
    await page.keyboard.press('Tab');
    if (await page.evaluate((s) => document.activeElement?.matches(s) ?? false, selector)) return;
  }
  throw new Error(`focus introuvable : ${selector}`);
}

/** Mesures prises dans la page : elles disent ce que les captures montrent. */
function measure(page) {
  return page.evaluate(() => {
    const box = (s) => {
      const r = document.querySelector(s)?.getBoundingClientRect();
      return r ? { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width * 10) / 10, h: Math.round(r.height * 10) / 10 } : null;
    };
    const cut = (s) => [...document.querySelectorAll(s)].filter((e) => e.scrollWidth > e.clientWidth).length;
    const wrapped = (s) => [...document.querySelectorAll(s)].filter((e) => e.getBoundingClientRect().height > 22).length;
    const meet = (a, b) => {
      const p = document.querySelector(a)?.getBoundingClientRect();
      const q = document.querySelector(b)?.getBoundingClientRect();
      return p && q ? p.left < q.right && q.left < p.right && p.top < q.bottom && q.top < p.bottom : null;
    };
    const bg = (s) => {
      const e = document.querySelector(s);
      return e ? getComputedStyle(e).backgroundColor : null;
    };
    return {
      viewport: { w: innerWidth, h: innerHeight },
      pageOverflows: document.documentElement.scrollHeight > innerHeight,
      panel: box('.nettoyage'),
      fetchButton: box('header .btn.primary'),
      closeButton: box('header .btn.ghost'),
      filter: box('.filtre'),
      row: box('.ligne'),
      checkbox: box('.ligne input'),
      footerButton: box('footer .btn'),
      summary: box('.resume'),
      namesCut: cut('.nom'),
      countsWrapped: wrapped('.commits'),
      listScrolls: (() => {
        const e = document.querySelector('.liste');
        return e ? e.scrollHeight > e.clientHeight : null;
      })(),
      confirmation: box('.confirmation'),
      confirmationOverFooter: meet('.confirmation', 'footer'),
      confirmationOverList: meet('.confirmation', '.liste'),
      confirmationOverSummary: meet('.confirmation', '.resume'),
      selectedBackground: bg('.ligne.sel'),
      hoveredBackground: bg('.ligne:hover'),
      focused: document.activeElement?.textContent?.trim().slice(0, 30) ?? null,
    };
  });
}

(async () => {
  mkdirSync(out, { recursive: true });
  const server = spawn('npx', ['vite', '--port', String(PORT), '--strictPort'], { cwd: root, stdio: 'ignore' });
  const browser = await chromium.launch();
  const measures = {};
  try {
    await ready();
    for (const theme of ['clair', 'sombre']) {
      for (const [size, width] of [['large', 760], ['etroit', 440]]) {
        const context = await browser.newContext({
          viewport: { width, height: 600 },
          deviceScaleFactor: 2,
          colorScheme: theme === 'sombre' ? 'dark' : 'light',
          reducedMotion: 'reduce',
        });
        const page = await context.newPage();
        await page.goto(PAGE);
        await page.waitForSelector('.ligne');
        await page.evaluate(() => document.fonts.ready);
        const shot = async (name) => {
          const file = `${name}-${theme}.png`;
          await page.screenshot({ path: join(out, file) });
          measures[file] = await measure(page);
        };

        // 1. Au repos : deux lignes cochées au clavier, l'anneau sur la ligne active, une ligne survolée.
        await tabTo(page, '.liste');
        await page.keyboard.press('Space');
        await page.keyboard.press('ArrowDown');
        await page.keyboard.press('ArrowDown');
        await page.keyboard.press('Space');
        await page.locator('.ligne').nth(4).hover();
        await shot(size === 'large' ? '01-repos' : '04-etroit');

        if (size === 'large') {
          // 2. La confirmation, ouverte au clavier.
          await page.mouse.move(2, 2);
          await page.focus('footer .btn.primary');
          await page.keyboard.press('Enter');
          await page.waitForSelector('.confirmation');
          await shot('02-confirmation');

          // 3. La suppression échoue, puis « Fetch » est lancé : erreur, attente, boutons désactivés.
          await page.click('.confirmation .btn.danger');
          await page.waitForSelector('.erreur');
          await page.click('header .btn.primary');
          await page.mouse.move(2, 2);
          await page.waitForTimeout(150);
          await shot('03-erreur');
        }
        await context.close();
      }
    }
  } finally {
    await browser.close();
    server.kill();
  }
  writeFileSync(join(out, 'mesures.json'), JSON.stringify(measures, null, 2) + '\n');
  console.log(`Captures et mesures écrites dans ${out}`);
})();
