# 首页三方一致性（网页原型 ↔ design.md ↔ 游戏实现）

**Status: active**
**原则：设计优先，以网页原型（`web/index.html` + `web/style.css` + `web/scale.js`）为唯一设计基准。**
design.md 与游戏实现（`src/ui_resources/` 及菜单代码）凡与网页原型冲突处，均以网页原型为准修改。

## 背景

2026-10-22 三方对比发现 8 处差异，经用户裁决归为以下处置：

| # | 差异 | 裁决 | 去向 |
| --- | --- | --- | --- |
| 1 | 窗口适配：游戏保留 640/900 断点 + 窄屏逐控件缩小；网页已改为整体 UiScale | 网页为准 | [issues/01](issues/01-game-uniform-scale.md) |
| 2 | 菜单分隔线：网页 sRGB alpha 0.15，游戏线性补偿 0.028 | 网页为准 | [issues/02](issues/02-separator-alpha.md) |
| 3 | 标题字重：design.md 声明 700，网页实现 900 | 网页为准 | [issues/03](issues/03-design-md-token-sync.md) |
| 4 | 标题载体：游戏 PNG 美术字 vs 网页 CSS 渐变 | 并存，非冲突 | 不拆票（见下） |
| 5 | 版本字号双轨：design.md 11.2px，实现均 10.88px | 网页为准（工单 04 后网页为 10px，design.md 与游戏实现按 10px 同步） | [issues/03](issues/03-design-md-token-sync.md) |
| 6 | 网页页脚随 zoom 缩放，design.md 要求 HUD 锚点独立适配 | 不冲突：该规则针对战斗 HUD，菜单页脚属菜单体系 | 不拆票（见下） |
| 7 | 场景交接 600ms 淡出/淡入在网页原型缺失 | 原型阶段边界 | 不拆票（见下） |
| 8 | style.css rem/em 残留 | 网页自身收敛 | [issues/04](issues/04-css-fixed-px.md) |

## 不拆票边界（记录，不行动）

- **#4 标题载体**：游戏标题 PNG 是烘焙美术资源；网页为设计基准后，PNG 若需对齐网页字重属美术重制工作，不属代码工单，待美术排期。
- **#6 页脚缩放**：design.md 中 HUD 锚点规则仅约束战斗 HUD；菜单页脚（`hud--bottom`）属菜单界面，随整体缩放是正确行为，design.md 表述已足够，无需改动。
- **#7 场景交接**：600ms 淡出/黑场/淡入属游戏场景切换，网页原型无对应场景；待游戏实现时按 design.md 执行。
