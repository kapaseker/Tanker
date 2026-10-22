# 01 游戏菜单窗口适配改为整体 UiScale 等比缩放

**Status: ready-for-agent**
**Blocked by: 无**

## 背景

设计基准（`web/scale.js` + `web/style.css`）已采用游戏思路的窗口适配：控件尺寸为固定设计常量（1280px 参考分辨率），窗口窄于参考值时整体等比缩小，无布局断点。

游戏菜单实现仍是响应式网页思路：[dimens.rs](file:///e:/Workspace/Code/Rust/Tanker/src/ui_resources/dimens.rs) 保留 `NARROW_BREAKPOINT 640` / `COMPACT_BREAKPOINT 900`，窄屏时标签缩到 17.6px、900px 以下收起装饰插图。

## 目标

- 移除 `NARROW_BREAKPOINT` / `COMPACT_BREAKPOINT` 及其分支逻辑（窄屏标签字号、装饰插图收起等）
- 菜单所有控件尺寸取网页原型对应设计常量（1280px 参考分辨率下的值）
- 新增/复用整体缩放机制：窗口窄于 1280 逻辑宽度时对 UI 根节点施加等比 `UiScale`（等价 scale.js 的 zoom 语义：`可用宽度 / 1280`），高度不参与计算
- 窗口宽于 1280 时不放大（与 scale.js 一致，布局容器自然居中吸收）

## 验收

- 任意窗口尺寸下菜单控件尺寸与 1280px 视口下的网页原型渲染一致（等比意义）
- 离屏/程序内无按断点分支的菜单代码残留
- `cargo check` 通过；涉及画面的改动用离屏入口截图核验
