# ADR-0003：抽取共享 UI 装饰控件层 src/ui/widgets

- 状态：已采纳
- 日期：2026-10-23

## 背景

首页与画廊各自重复实现「直角角标」和「右下角版本号」，样式与定位分散在两页的 spawn 与布局 system 中。画廊版本号缺 2px 字距，并用视口尺寸硬算（`size.x-148`）定位，版本字符串或字距变化时右下角会漂移。

## 决策

- 新增 `src/ui/widgets` 共享控件层，提供两个控件：
  - `FrameCorner`：左上或右下的 L 形直角边，颜色作参数，尺寸/边框宽取 `dimens::index::CORNER_SIZE`、`dimens::BORDER_WIDTH`。
  - `VersionBadge`：版本号文字 + 右下直角边的组合控件，文字统一首页规格（`BUILD_SIZE`/`BUILD_SPACING` 2px 字距/`LINE_HEIGHT`/`ON_SURFACE_MUTED`），按 `BUILD_RIGHT`/`BUILD_BOTTOM` 锚点定位。
- 采用「marker 组件 + 单一状态无关布局 system」：`layout_frame` 只按 marker 设静态锚点；字号、字距、行高、边框、颜色等样式在共享 spawn 辅助里一次写好。
- 遵守 ADR-0002：`widgets` 只依赖 bevy 与 `ui::resources`，不引用页面、游戏状态或验证入口；system 常驻 `Update`，无 marker 实体时空跑。
- 角标颜色保留各页现状（首页 `colors::index::CORNER`、画廊 `colors::gallery::OUTLINE_ACTIVE`），使首页成为外观不变的纯重构；画廊版本号统一到首页规格，顺带修复缺字距与脆弱定位的偏差。

## 最终检查与取舍

- `layout_frame` 每帧重设静态锚点，与两页既有布局 system 同模式；控件若未来需要动画或响应尺寸，再升级为按需更新。
- 控件实体是全屏 root 的子节点，随页面 `OnExit` 递归销毁，不单独清理。
- 不改 `design.md`：画廊版本号是向既有 `index.decorative-meta`（10px / 2px 字距）规范靠拢，属修复而非新增设计规则。
- 验证：`cargo check`/`cargo test` 通过；`--offscreen-test index` 角标与版本号外观不变，`--offscreen-test gallery` 版本号补齐字距并贴齐右下角（right 54 / bottom 30）。
