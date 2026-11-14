// Regenerates the icons of the application (src-tauri/icons) from the logo (docs/logo).
//
// Run it again only when the drawing of the logo changes:
//   node scripts/generate-icons.cjs
//
// A one-off tool, outside the dependencies of the project. It needs Playwright and its Chromium
// (`npm install --no-save playwright && npx playwright install chromium`), which draw each size
// directly from the SVG.
//
// Rule of the guidelines (section "Logo"): the full drawing from 40 px up, the simplified drawing
// below.
const { execFileSync } = require('node:child_process');
const { mkdtempSync, readFileSync, writeFileSync, copyFileSync, rmSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join, resolve } = require('node:path');
const { chromium } = require('playwright');

const root = resolve(__dirname, '..');
const icons = join(root, 'src-tauri', 'icons');
const full = readFileSync(join(root, 'docs', 'logo', 'ramure-icon.svg'), 'utf8');
const small = readFileSync(join(root, 'docs', 'logo', 'ramure-icon-small.svg'), 'utf8');
const SMALL_BELOW = 40;

/** The PNG files Tauri expects, with their size in pixels. */
const PNG = {
  '32x32.png': 32,
  '128x128.png': 128,
  '128x128@2x.png': 256,
  'icon.png': 512,
  'Square30x30Logo.png': 30,
  'Square44x44Logo.png': 44,
  'Square71x71Logo.png': 71,
  'Square89x89Logo.png': 89,
  'Square107x107Logo.png': 107,
  'Square142x142Logo.png': 142,
  'Square150x150Logo.png': 150,
  'Square284x284Logo.png': 284,
  'Square310x310Logo.png': 310,
  'StoreLogo.png': 50,
};
/** The sizes gathered in icon.ico, for Windows. */
const ICO = [16, 24, 32, 48, 64, 256];

/** Assembles an .ico file whose every image is a PNG, as Tauri does. */
function ico(images) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(1, 2); // type: icon
  header.writeUInt16LE(images.length, 4);
  const directory = Buffer.alloc(16 * images.length);
  let offset = header.length + directory.length;
  images.forEach(({ size, data }, index) => {
    const entry = index * 16;
    directory.writeUInt8(size >= 256 ? 0 : size, entry); // 0 means 256
    directory.writeUInt8(size >= 256 ? 0 : size, entry + 1);
    directory.writeUInt16LE(1, entry + 4); // planes
    directory.writeUInt16LE(32, entry + 6); // bits per pixel
    directory.writeUInt32LE(data.length, entry + 8);
    directory.writeUInt32LE(offset, entry + 12);
    offset += data.length;
  });
  return Buffer.concat([header, directory, ...images.map((image) => image.data)]);
}

(async () => {
  const browser = await chromium.launch();
  const render = async (size) => {
    const page = await browser.newPage({ viewport: { width: size, height: size }, deviceScaleFactor: 1 });
    const svg = (size < SMALL_BELOW ? small : full).replace('<svg ', `<svg width="${size}" height="${size}" `);
    await page.setContent(`<!doctype html><style>html,body{margin:0;background:transparent}svg{display:block}</style>${svg}`);
    const data = await page.screenshot({ omitBackground: true, clip: { x: 0, y: 0, width: size, height: size } });
    await page.close();
    return data;
  };

  for (const [name, size] of Object.entries(PNG)) writeFileSync(join(icons, name), await render(size));

  const layers = [];
  for (const size of ICO) layers.push({ size, data: await render(size) });
  writeFileSync(join(icons, 'icon.ico'), ico(layers));

  // The .icns (macOS) is made by the command of Tauri, from a large image.
  const work = mkdtempSync(join(tmpdir(), 'ramure-icons-'));
  writeFileSync(join(work, 'source.png'), await render(1024));
  await browser.close();
  execFileSync('npx', ['tauri', 'icon', join(work, 'source.png'), '--output', join(work, 'out')], { cwd: root, stdio: 'ignore' });
  copyFileSync(join(work, 'out', 'icon.icns'), join(icons, 'icon.icns'));
  rmSync(work, { recursive: true, force: true });

  console.log(`Icons regenerated in ${icons}`);
})();
