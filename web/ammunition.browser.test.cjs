// 后台浏览器验收；同 transition.test.cjs，先从工程根目录启动 HTTP 服务。
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const { mkdir } = require('node:fs/promises');

(async () => {
  const browser = await chromium.launch({
    executablePath: process.env.WEB_BROWSER || '/opt/microsoft/msedge/msedge', headless: true,
  });
  try {
    const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    const base = process.env.WEB_TEST_URL || 'http://127.0.0.1:8765/web/';
    await mkdir('.scratch/ammunition-web', { recursive: true });
    const screenshot = name => page.screenshot({ path: `.scratch/ammunition-web/${name}.png` });
    const labels = () => page.locator('.spec__label').allTextContents();
    await page.goto(base + 'model-detail.html?c=0&m=0');
    await page.locator('.specs[aria-busy="false"]').waitFor();
    assert.deepEqual(await labels(), ['初速', '射速']);
    assert.deepEqual(await page.locator('.spec__value').allTextContents(), ['1200', '12']);
    await screenshot('apfsds');
    await page.goto(base + 'model-detail.html?c=1&m=0');
    assert.deepEqual(await labels(),
      ['前进速度', '倒车速度', '水平旋转', '炮管俯仰范围', '水平转速', '俯仰速度']);
    assert.deepEqual(await page.locator('.spec__value').allTextContents(),
      ['16', '8', '全圈', '-9°～+20°', '30°', '9°']);
    assert.ok(await page.locator('#aim-help').isVisible());
    await screenshot('tank');
    await page.goto(base + 'gameplay.html');
    await page.locator('#reload[aria-busy="false"]').waitFor();
    assert.equal(await page.locator('.bar__ticks > span').count(), 4);
    assert.equal(await page.locator('#reload').getAttribute('aria-valuenow'), '100');
    await screenshot('hud');

    // 修改响应而非磁盘配置，证明详情与HUD从同一文件动态读取，非编译常量。
    await page.route('**/bullet.jsonc', route => route.fulfill({
      body: '{// 调整弹药\n"muzzle_velocity":1500,"rate_of_fire":24,"collision":{"size":[0.72,0.04,0.04],"center_forward_offset":0.16}}',
      contentType: 'application/json',
    }));
    await page.goto(base + 'model-detail.html?c=0&m=0');
    await page.locator('.specs[aria-busy="false"]').waitFor();
    assert.deepEqual(await page.locator('.spec__value').allTextContents(), ['1500', '24']);
    await page.goto(base + 'gameplay.html');
    await page.locator('#reload[aria-busy="false"]').waitFor();
    assert.equal(await page.locator('.bar__ticks > span').count(), 2);
    await page.keyboard.press('Space');
    await page.waitForFunction(() => Number(document.querySelector('#reload').getAttribute('aria-valuenow')) < 50);
    await page.waitForTimeout(600);
    const progress = Number(await page.locator('#reload').getAttribute('aria-valuenow'));
    assert.ok(progress > 10 && progress < 50);
    await page.keyboard.press('Space');
    assert.ok(Number(await page.locator('#reload').getAttribute('aria-valuenow')) >= progress);
    await page.waitForFunction(() => document.querySelector('#reload').getAttribute('aria-valuenow') === '100');

    await page.unroute('**/bullet.jsonc');
    await page.route('**/bullet.jsonc', route => route.fulfill({ status: 404, body: '' }));
    await page.goto(base + 'gameplay.html');
    await page.locator('[role="alert"]').waitFor();
    assert.match(await page.locator('[role="alert"]').textContent(), /弹药配置加载失败.*404/);
    await page.keyboard.press('Space');
    assert.equal(await page.locator('#reload').getAttribute('aria-valuenow'), '0');
    await page.goto(base + 'model-detail.html?c=0&m=0');
    await page.locator('[role="alert"]').waitFor();
    assert.deepEqual(await labels(), []);
    assert.ok(await page.locator('#detail-back').isVisible());
    assert.deepEqual(errors, []);
    console.log('浏览器验收通过：配置同源、弹药/坦克规格、动态冷却分格、连点冷却、缺配置失败');
  } finally {
    await browser.close();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
