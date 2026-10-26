// 后台浏览器回归：NODE_PATH 指向已有 Playwright，WEB_BROWSER 可指定浏览器。
// 先在工程根目录启动 python3 -m http.server 8765 --bind 127.0.0.1。
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const { mkdir, writeFile } = require('node:fs/promises');

(async () => {
  const browser = await chromium.launch({
    executablePath: process.env.WEB_BROWSER || '/opt/microsoft/msedge/msedge', headless: true,
  });
  try {
    const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    const base = process.env.WEB_TEST_URL || 'http://127.0.0.1:8765/web/';
    const phase = () => page.evaluate(() => document.documentElement.dataset.transition);
    const idle = () => page.waitForFunction(() => document.documentElement.dataset.transition === 'idle');
    const darkness = () => page.evaluate(() => Number(getComputedStyle(document.querySelector('.page-fade')).opacity));
    const title = () => page.locator('.backbar__title').textContent();
    const screenshot = name => page.screenshot({ path: `.scratch/web-transition/${name}.png` });
    await mkdir('.scratch/web-transition', { recursive: true });

    await page.goto(base + 'index.html');
    await idle();
    assert.equal(await title(), '主菜单');
    assert.equal(await page.locator('.back').isVisible(), true);
    assert.equal(await page.locator('.back').getAttribute('aria-label'), '退出游戏');
    assert.equal(await page.locator('.back__arrow').count(), 1);
    await screenshot('home');
    // 首页 ESC 触发退出：淡出黑场后显示提示，再按 ESC 淡入返回主菜单。
    // 屏蔽真实 window.close，保证不同浏览器下退出流程可复现。
    await page.evaluate(() => { window.close = () => {}; });
    await page.keyboard.press('Escape');
    assert.equal(await phase(), 'exit');
    await page.waitForSelector('.exit-note');
    assert.equal(await darkness(), 1);
    await screenshot('home-exit');
    await page.keyboard.press('Escape');
    await idle();
    assert.equal(await page.locator('.exit-note').count(), 0);
    assert.equal(await darkness(), 0);
    const corner = await page.screenshot({ clip: { x: 18, y: 18, width: 23, height: 23 } });
    const titleBox = await page.locator('.backbar__title').boundingBox();

    const entry = page.locator('.menu__item[href="model-gallery.html"]');
    await entry.focus();
    await page.keyboard.press('Enter');
    assert.equal(await title(), '模型画廊');
    assert.equal(await page.locator('.back').getAttribute('aria-label'), '取消页面切换');
    await page.waitForTimeout(250);
    assert.ok(await darkness() > 0.1 && await darkness() < 0.9);
    await screenshot('fade-out');
    assert.deepEqual(await page.screenshot({ clip: { x: 18, y: 18, width: 23, height: 23 } }), corner);
    const targetTitleBox = await page.locator('.backbar__title').boundingBox();
    for (const key of ['x', 'y', 'height']) assert.equal(targetTitleBox[key], titleBox[key]);
    await page.keyboard.press('Escape');
    assert.equal(await phase(), 'idle');
    assert.equal(await darkness(), 0);
    assert.equal(await title(), '主菜单');
    assert.equal(await page.locator('.back').getAttribute('aria-label'), '退出游戏');
    assert.equal(await entry.evaluate(el => el === document.activeElement), true);
    await page.waitForTimeout(650);
    assert.ok(page.url().endsWith('index.html'));

    await entry.click();
    await page.locator('.back').click();
    assert.equal(await phase(), 'idle');
    assert.ok(page.url().endsWith('index.html'));
    await entry.click();
    // 重复正文点击不得提交第二次导航。
    await page.mouse.click(300, 420);
    await page.waitForURL('**/model-gallery.html');
    await page.waitForFunction(() => document.documentElement.dataset.transition === 'in' && document.getAnimations().length > 0);
    await page.waitForTimeout(180);
    await screenshot('fade-in');
    const before = await darkness();
    await page.keyboard.press('Escape');
    assert.equal(await phase(), 'out');
    assert.equal(await title(), '主菜单');
    assert.ok(await darkness() >= before - 0.08);
    await page.waitForURL('**/index.html');
    await idle();
    assert.equal(await entry.evaluate(el => el === document.activeElement), true);

    await page.goto(base + 'model-gallery.html');
    await idle();
    await page.evaluate(() => scrollTo(0, 420));
    const scroll = await page.evaluate(() => scrollY);
    await page.locator('.back').click();
    await page.waitForTimeout(150);
    await page.locator('.back').click();
    assert.equal(await title(), '模型画廊');
    assert.equal(await page.evaluate(() => scrollY), scroll);
    await screenshot('gallery-cancel');
    await page.locator('.back').click();
    await page.keyboard.down('Escape');
    await page.keyboard.down('Escape');
    await page.keyboard.up('Escape');
    assert.equal(await phase(), 'idle');
    await page.waitForTimeout(650);
    assert.ok(page.url().endsWith('model-gallery.html'));

    // 延迟目标文档响应，读取正式导航等待期间的全黑内容与固定装饰。
    const cdp = await page.context().newCDPSession(page);
    await page.route('**/web/index.html', async route => {
      await new Promise(resolve => setTimeout(resolve, 1800));
      await route.continue();
    });
    await page.keyboard.press('Escape');
    await page.waitForTimeout(650);
    const black = await cdp.send('Page.captureScreenshot');
    await writeFile('.scratch/web-transition/black-boundary.png', Buffer.from(black.data, 'base64'));
    await cdp.detach();
    await page.waitForURL('**/index.html');
    await idle();
    await page.unroute('**/web/index.html');

    await page.goto(base + 'model-detail.html?c=0&m=0');
    await idle();
    assert.notEqual(await title(), '—');
    const target = await page.locator('.back').getAttribute('href');
    await page.keyboard.press('Escape');
    await page.waitForURL(url => url.href.endsWith(target));
    await idle();
    assert.ok(new URL(page.url()).hash);
    await page.reload();
    await idle();
    await page.goBack();
    await idle();
    assert.ok(page.url().includes('model-detail.html'));

    for (const width of [1280, 800]) {
      await page.setViewportSize({ width, height: 720 });
      await page.goto(base + 'index.html');
      await idle();
      const bar = await page.locator('.backbar').boundingBox();
      const content = await page.locator('.hero').boundingBox();
      assert.ok(content.y >= bar.y + bar.height);
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
      await screenshot(`home-${width}`);
    }
    const noScript = await browser.newPage({ javaScriptEnabled: false });
    await noScript.goto(base + 'index.html');
    await noScript.locator('.menu__item[href="model-gallery.html"]').click();
    assert.ok(noScript.url().endsWith('model-gallery.html'));
    assert.deepEqual(errors, []);
    console.log('PASS: 切页、Esc/鼠标取消、首页退出与返回、淡入中返回、滚动/焦点、详情锚点、刷新/后退、缩放和无JS导航');
  } finally {
    await browser.close();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
