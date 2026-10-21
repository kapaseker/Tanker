// 从工程设计稿导出标题与其 CSS 渐变/挤出/阴影；运行环境需要 Playwright。
// NODE_PATH 指向已有 Playwright 安装，不为游戏增加运行时依赖。
const { chromium } = require('playwright');
const path = require('node:path');
const fs = require('node:fs');
const assert = require('node:assert/strict');
const { pathToFileURL } = require('node:url');
(async () => {
  const browser = await chromium.launch({ headless: true,
    ...(process.env.MENU_BROWSER ? { executablePath: process.env.MENU_BROWSER } : {}) });
  try {
    const page = await browser.newPage({ viewport: { width: 1200, height: 400 }, deviceScaleFactor: 2 });
    await page.goto(pathToFileURL(path.resolve('web/index.html')).href);
    await page.evaluate(() => document.fonts.ready);
    const bounds = await page.evaluate(() => {
      const title = document.querySelector('.title');
      document.body.replaceChildren(title);
      document.body.style.cssText = 'display:block;background:transparent;min-height:0';
      Object.assign(title.style, { position:'absolute', left:'80px', top:'40px', fontSize:'176px', width:'max-content' });
      const r = title.getBoundingClientRect();
      return { width:r.width, height:r.height };
    });
    assert(Math.abs(bounds.width - 736.046875) < 1, '标题字宽异常：请确认 MiSans 已实际加载');
    fs.mkdirSync('assets/menu', { recursive:true });
    await page.screenshot({ path:'assets/menu/title.png', omitBackground:true,
      clip:{ x:0, y:0, width:Math.ceil(bounds.width+160), height:Math.ceil(bounds.height+130) } });
    console.log(bounds);
  } finally { await browser.close(); }
})();
