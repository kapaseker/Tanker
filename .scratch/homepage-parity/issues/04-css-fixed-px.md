# 04 style.css rem/em 残留收敛为固定 px 设计常量

**Status: resolved**
**Blocked by: 无**

## 背景

设计基准自身存在单位残留：`web/style.css` 仍有 rem/em 值（如 `0.68rem`、`padding: 1.05rem 0.35rem`、`gap: 1rem` 等）。整体缩放方案（scale.js zoom）下，rem 相对浏览器根字号解析，与"固定设计常量 + 整体缩放"的模型不符；且用户明确偏好固定 px（偶数取整）。

## 目标

- 遍历 style.css，将全部 rem/em 值换算为 px（rem 按 16px 根字号换算，四舍五入取偶数）
- 换算后的值与换算前渲染几何逐项一致（仅允许取整带来的 ≤1px 偏差）
- 不改动任何布局结构、选择器与动画

## 验收

- `grep -E '[0-9.]+(rem|em)' web/style.css` 无字号/间距类残留（字体族名 `--font-*` 等非尺寸项除外）
- 本地服务实测：换算前后页面关键元素 getBoundingClientRect 尺寸一致

## Comments

- 2026-10-22 完成。web/style.css 全量换算为偶数 px，grep 验收零残留（含 web/*.html、*.js 复查亦无残留）；px 值全为偶数，仅 1px 细线（边框/渐变线/扫描线）按用户裁决保留。
- 用户四项换算裁决：① 0.68rem→10px（就近偶，非 12px）；② 奇数中心值统一向上取偶（kbd 字距≈1px→2px、kbd 内边距≈2.8/5px→4px/6px、eyebrow 字距≈3px→4px、7px 方点→8px、3px 圆角→4px、5px 箭头位移→6px）；③ 0.02em 微字距（0.36–0.48px）归 0 删除；④ 存量小数 px（30.4/54.4/32.4/76.8/153.6/15.2/345.6）就近取偶（→30/54/32/76/154/16/346）。
- 无单位 line-height（0.84/1.75/1.2/1）按"比率非长度单位"保留，用户未提出异议。
- 渲染几何验证（viewport 1465×1190，改前基线见 `04-baseline-before.json`）：结构值偏差全部 ≤1px（.viewer 345.59→346、.hud 高 60.78→60）；kbd 因向上取偶增高 2.47px（用户裁决预期内）；文字宽度变化均可用"字号+字距逐字符取整叠加"精确解释（如 .backbar__title −1.61px = 0.4px 字距 × 4 字符）。对照组（.voxels/.frame/.card__stage）完全不变。
- 验证注意事项：浏览器会缓存 style.css，复测须换端口（8123→8124）并用 computed style 自检（kbd padding "4px 6px" 等）确认新 CSS 生效。
- 遗留关联：web 端 .hud 字号现为 10px，spec #5 裁决记录的 10.88px 已过时；工单 03（未执行）执行时 design.md 应同步为 10px，游戏实现后续同样需按网页基准对齐。
