# ADR-0004：统一标题栏控件 widgets::title_bar 与内建返回交互

- 状态：已采纳
- 日期：2026-10-24

## 背景

画廊页手写标题栏：头部矩形每帧写视口宽度、ESC 返回框命中与悬停动画、标题定位分散在 `pages/gallery/ui.rs` 的 spawn、input 与 update 三处，常量（HEADER_HEIGHT/BACK_RECT/TITLE_RECT 等）挂在 `dimens::gallery`。后续内容页（设置、详情等）需要同样的"统一高度 + ESC 返回框 + 标题 + 全屏宽底部分割线"，不能各页再抄一份。

## 决策

- 新增 `src/ui/widgets/title_bar.rs` 控件：`spawn_title_bar` 一次生成头部（`width: percent(100)`、高 88 逻辑像素、1px 底边框即分割线）、ESC 返回框（弹性布局，复用 `spawn_keycap`）与页面标题；尺寸常量收敛到 `dimens::title_bar`，配色收敛到 `colors::title_bar`。
- 交互全部内建于控件（用户裁决）：悬停、点击按下/松开同框、ESC、Tab 聚焦 + Enter 与 220ms 颜色过渡都在控件的 `interaction` 系统内；激活时发 `TitleBarBackActivated` 消息，页面只读取消息并决定去向与过渡锁。控件不持有页面或游戏状态，仍满足 ADR-0002。
- 交互状态放全局 `TitleBarState` 资源（用户裁决）：`pointer` 字段 `pub(crate)`，窗口模式每帧被真实光标覆盖，离屏验证直接注入替代旧的 `View.pointer` 注入。基于"同屏最多一个标题栏"的当前事实。
- `zoom()` 从 `pages::index` 上移到 `ui::scaling`（`ui::zoom`）：控件需要窗口→逻辑像素换算，且缩放本就是 UI 层职责，避免 widgets 反向依赖 pages。
- `widgets::eased`/`text_font` 提升为共享辅助，画廊卡片悬停动画继续复用；画廊删除 Part::{Header,Back,Title}、`View` 的返回框字段与相应 update/input 分支。
- `design.md` 新增 `title-bar.height`（88px）token 与"内容页共用统一标题栏控件"规则（用户裁决：统一高度进规范）。

## 最终检查与取舍

- 头部宽度用 `percent(100)` 替代旧的每帧手写 `size.x`：依赖整体缩放下 Taffy 布局视口等于逻辑视口。若未来出现 1px 级偏差，回退方案是控件保留 marker、几何交回页面布局系统写宽度。
- 标题栏存在期间 ESC 被 `interaction` 固定消费为"返回"；未来页面需要 ESC 做其它事时再调整控件协议。
- 消息读取可能比控件写入晚一帧（同在 Update，系统顺序未定）；页面过渡有 2000 帧量级余量，无实际影响。
- `colors::title_bar::OUTLINE` 当前与 `colors::gallery::OUTLINE` 同值但独立演化，避免跨页面共享语义被单页改动劫持。
- 验证：`cargo check`/`cargo test`（63 项，含返回框按下/松开配对的控件化测试）通过；`--offscreen-test gallery` 退出码 0，`gallery-top.png` 标题栏外观与改动前一致，`gallery-back-hover.png` 返回框高亮与键帽描边正常，过渡与三种窗口宽度检查通过。
