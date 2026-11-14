// Takes the screenshots of the review test: the panel mounted on its own, in light and in dark,
// in four states, with the measures taken in the page.
//
//   node .claude/tests/design-reviewer/capture/capture.cjs <output folder>
//
// Run it once the change is copied into src/lib/git/ (see the README of the test).
// A one-off tool, outside the dependencies of the project: it needs Playwright and its Chromium.
const { spawn } = require('node:child_process');
const { mkdirSync, writeFileSync } = require('node:fs');
const { join, resolve } = require('node:path');
const { chromium } = require('playwright');

const root = resolve(__dirname, '../../../..');
const out = resolve(process.argv[2] ?? 'screens');
const PORT = 5199;
const PAGE = `http://localhost:${PORT}/.claude/tests/design-reviewer/capture/index.html`;

/** Waits until the development server answers. */
async function ready() {
  for (let i = 0; i < 120; i++) {
    try {
      if ((await fetch(PAGE)).ok) return;
    } catch {}
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error('the development server does not answer');
}

/** Tabs to the wanted element: the focus then comes from the keyboard, and its ring shows. */
async function tabTo(page, selector) {
  for (let i = 0; i < 20; i++) {
    await page.keyboard.press('Tab');
    if (await page.evaluate((s) => document.activeElement?.matches(s) ?? false, selector)) return;
  }
  throw new Error(`focus not found: ${selector}`);
}

/** Measures taken in the page: they say what the screenshots show. */
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
      panel: box('.cleanup'),
      fetchButton: box('header .btn.primary'),
      closeButton: box('header .btn.ghost'),
      filter: box('.filter'),
      row: box('.row'),
      checkbox: box('.row input'),
      footerButton: box('footer .btn'),
      summary: box('.summary'),
      namesCut: cut('.name'),
      countsWrapped: wrapped('.commits'),
      listScrolls: (() => {
        const e = document.querySelector('.list');
        return e ? e.scrollHeight > e.clientHeight : null;
      })(),
      confirmation: box('.confirmation'),
      confirmationOverFooter: meet('.confirmation', 'footer'),
      confirmationOverList: meet('.confirmation', '.list'),
      confirmationOverSummary: meet('.confirmation', '.summary'),
      selectedBackground: bg('.row.sel'),
      hoveredBackground: bg('.row:hover'),
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
    for (const theme of ['light', 'dark']) {
      for (const [size, width] of [['wide', 760], ['narrow', 440]]) {
        const context = await browser.newContext({
          viewport: { width, height: 600 },
          deviceScaleFactor: 2,
          colorScheme: theme,
          reducedMotion: 'reduce',
        });
        const page = await context.newPage();
        await page.goto(PAGE);
        await page.waitForSelector('.row');
        await page.evaluate(() => document.fonts.ready);
        const shot = async (name) => {
          const file = `${name}-${theme}.png`;
          await page.screenshot({ path: join(out, file) });
          measures[file] = await measure(page);
        };

        // 1. At rest: two rows checked from the keyboard, the ring on the active row, a row hovered.
        await tabTo(page, '.list');
        await page.keyboard.press('Space');
        await page.keyboard.press('ArrowDown');
        await page.keyboard.press('ArrowDown');
        await page.keyboard.press('Space');
        await page.locator('.row').nth(4).hover();
        await shot(size === 'wide' ? '01-rest' : '04-narrow');

        if (size === 'wide') {
          // 2. The confirmation, opened from the keyboard.
          await page.mouse.move(2, 2);
          await page.focus('footer .btn.primary');
          await page.keyboard.press('Enter');
          await page.waitForSelector('.confirmation');
          await shot('02-confirmation');

          // 3. The deletion fails, then the fetch starts: an error, a wait, disabled buttons.
          await page.click('.confirmation .btn.danger');
          await page.waitForSelector('.error');
          await page.click('header .btn.primary');
          await page.mouse.move(2, 2);
          await page.waitForTimeout(150);
          await shot('03-error');
        }
        await context.close();
      }
    }
  } finally {
    await browser.close();
    server.kill();
  }
  writeFileSync(join(out, 'measures.json'), JSON.stringify(measures, null, 2) + '\n');
  console.log(`Screenshots and measures written to ${out}`);
})();
