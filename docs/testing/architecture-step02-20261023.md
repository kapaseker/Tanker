# 应用组装与状态迁移验证（任务 02）

2026-10-23，基于 `2a55cb36ce460082c466fa0832788abca4d71a6e`，在当前 `master` 执行任务 02。
原有未跟踪的 `docs/plans/` 未修改、未纳入本次提交。

## 变更边界

- `main.rs` 仅保留模块声明、参数解析、调用启动入口和退出码处理。
- 应用组装、场景生成和加载汇报原样迁入 `app/mod.rs`；`run_game` 改为 `app::run`，仅向父模块公开。
- `GameState` 迁入 `app/state.rs`，`transition.rs` 迁入 `app/transition.rs`。
- 同步页面、驾驶、战斗、地形、离屏与现有测试的路径，以及 GLOSSARY 中三处定位；无旧路径重导出。
- `VoxScene` 保持 app 私有，子模块直接访问；体素尺度暂在 app，后续按模型契约任务集中。
- 本步骤只搬迁，不调整过渡、GPU 就绪、业务清理、插件注册、状态进入退出或系统调度。
  场景生成职责的进一步拆分留给任务 06，没有提前实施。

通过文本断言确认：应用启动函数（仅名称/可见性变化）、`setup_scene`、`report_vox_scene` 的原实现不变；
状态定义不变；过渡模块仅改变状态和 `VoxScene` 路径；地形模块仅改变过渡标记路径。
检查保存在本地 `relocation-check.txt`。没有新增行为或测试，复用原有验证边界。

## 命令与结果

原始证据：`.scratch/architecture-reorganization/evidence/20261023-ticket02-01/`（下称 `EVIDENCE`，仅本机保留）。

| 命令 | 退出码 | 结果及日志 |
|---|---|---|
| `cargo check --locked`（状态/过渡迁移后） | 0 | `check-state-transition.log` |
| `cargo test --locked app::transition::tests` | 0 | 3 项通过；`test-transition.log` |
| `cargo check --locked`（组装提取初次） | 101 | 发现一处 `crate::transition` 旧导入；`check-app.log` |
| `cargo check --locked`（修正导入后） | 0 | `check-app-fixed.log` |
| `cargo test --locked menu::tests` | 0 | 5 项通过；`test-menu.log` |
| `cargo test --locked gallery::` | 0 | 5 项通过；`test-gallery.log` |
| `cargo test --locked`（提取后） | 0 | 60 项通过；`test-all.log` |
| `cargo check --locked`（清除无关格式化差异、收窄启动函数可见性后） | 0 | `check-final.log` |
| `cargo test --locked`（最终源码） | 0 | 60 项通过，0 failed/ignored；`test-all-final.log` |
| `cargo run --locked -- --offscreen-test menu --output EVIDENCE/menu` | 0 | 三轮双向过渡、加载、清理与输入/布局断言通过；`menu/report.txt` |
| `cargo run --locked -- --offscreen-test gallery --output EVIDENCE/gallery` | 0 | 三轮往返、预览、滚动与资源清理通过；`gallery/report.txt` |
| `git diff --check` | 0 | 无空白错误 |

离屏完整命令、退出码与耗时另见 `offscreen-results.csv`。
Windows / PowerShell，NVIDIA GeForce RTX 4060 / Vulkan；继承 `RUST_LOG=warn`。
两份报告均记录种子 42、1280×720、16.666667ms；各预设另外覆盖其已有布局尺寸。
16 个已跟踪资产的前后 SHA-256 全部一致，见 `assets-before.sha256` / `assets-after.sha256`。
Cargo.toml、Cargo.lock、scripts 和 assets 无变更。

## 与任务 01 的画面对照

对照目录为 `20261023-ticket01-01/`。本次 menu 27 张、gallery 18 张 PNG，数量均与基线一致。
已读取四张并排联系表 `compare-menu-1/2.jpg`、`compare-gallery-1/2.jpg`，共 16 对关键帧：
首页、进入/返回的半黑与全黑阶段、游戏揭幕、画廊顶部/底部、自转、窄布局及返回首页。
观察中未见布局、字体、模型、相机或黑场衔接回归；缩略图观察不等于完整像素或窗口行为验证。

menu 的 17 个同名图片均逐像素相同；10 个文件名随等待帧数偏移，例如进入半揭幕从 54 帧变为 59 帧，
对应报告均为 `In alpha=0.5`。按截图顺序及报告阶段对齐后，13 张 frame 图片有 9 张逐像素相同，
其余 4 张平均每通道绝对差低于 0.021（0–255 范围）。gallery 的 18 张有 4 张逐像素相同，
其余平均每通道绝对差低于 0.036。完整数值见 `image-comparison.json`。
两次运行等待帧数不同，且背景/预览带动画，不能把帧号或像素差直接解释为行为回归；
本次结合原样迁移断言、场景断言及画面观察判断，不宣称逐像素完全一致。

## 已有问题与未覆盖项

- menu/gallery 仍输出 ICU4X 中文/日文分词模型缺失日志，均通过断言并正常退出；未修复该基线问题。
- Cargo 仍有增量缓存硬链接失败并回退复制的警告。
- 按任务范围没有重跑 Python、生成器或其余六个离屏入口；任务 01 已记录的 Python 编码与过期常量失败仍保留。
- 没有可见窗口、真实键鼠/显示缩放、其他显卡或 Linux 验证；没有新增依赖或修改资产。
