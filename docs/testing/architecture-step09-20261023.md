# 任务 09：无窗口验证工具拆分验收

2026-10-23，从 `f0bcfb7` 开始，当前分支 `master`。范围为任务 09；没有新增依赖、改动资产或业务算法。

## 模块归属

- `src/validation/cli.rs`：原有参数、输出预检查、watchdog 启动方法。
- `src/validation/offscreen.rs`：无窗口插件、输入注入、事件证据、手动更新、GPU/资源就绪与超时线程。
- `src/validation/capture.rs`：截图视点、读回、像素断言、PNG 与报告排他写入。
- `src/validation/scenarios/`：公共帧表、输入脚本与计帧流程；scene 为真实地形/相机状态检查，drive/combat/wheels 为各帧行为断言，menu/gallery 为原过渡及页面验证。
- `src/validation/mod.rs`：入口导出和共享状态；原有 10 项测试按职责保留在对应模块。启动调用只改 `src/main.rs` 与 `src/app/mod.rs`。

保留原 CLI、报告/图片文件名、拒绝覆盖、失败退出、种子 42、1280×720、16_666_667 ns 步长、120 帧预热、120 秒 watchdog 和系统顺序。既有 reject.png 诊断行为未修改。

修改前检查了 src、scripts、docs 与 GLOSSARY 引用：生产引用仅启动入口；生成器不引用 offscreen；计划中的旧路径是迁移前问题说明，保留历史语义。未涉及页面设计实现。

## 命令与结果

原始证据目录 `.scratch/architecture-reorganization/evidence/20261023-ticket09-01/`（仅本机）。完整命令和退出码在 `results.jsonl`，各命令输出在同名 `.log`，各场景有 `report.txt`。

| 命令 / 入口 | 退出码 | 结果 |
|---|---|---|
| `cargo check --locked` | 0 | 最终复核日志 `final-check.log` |
| `cargo test --locked validation::` | 0 | 搬迁后的 10 项工具测试通过 |
| `cargo test --locked` | 0 | 63 passed；最终 `final-tests.log` |
| `cargo run --locked -- --capture EVIDENCE/capture.png` | 0 | 单图保存，无报告 |
| `--offscreen-test scene --output EVIDENCE/scene` | 0 | 场景/地形/相机 |
| `--offscreen-test drive --output EVIDENCE/drive` | 0 | 直行/转弯/履带 |
| `--offscreen-test combat --output EVIDENCE/combat` | 0 | 命中/爆炸/清理 |
| `--offscreen-test wheels --output EVIDENCE/wheels-player` | 0 | 玩家轮组 |
| `--offscreen-test wheels --output EVIDENCE/wheels-enemy --model enemy` | 0 | 敌人轮组 |
| `--offscreen-test menu --output EVIDENCE/menu` | 0 | 三轮双向过渡与清理 |
| `--offscreen-test gallery --output EVIDENCE/gallery` | 0 | 预览/滚动/自转/三轮清理 |

表中后七项同样使用 `cargo run --locked --` 前缀；EVIDENCE 为上述目录。GPU/后端均为 NVIDIA GeForce RTX 4060 / Vulkan。

全部 GPU 运行结束后的调整仅为内部可见性收窄、原有测试归属与去除无关 rustfmt 改动，随后重新执行 check 与完整测试。未重复 GPU 运行。

## 关键帧与基线对照

已读取 `review-01.jpg` 至 `review-08.jpg` 的 29 组基线/本轮关键帧对照：场景与驾驶、战斗 6/24/60/120 帧、双方轮组 0/30/90 帧和外包围框、菜单淡出/黑场/淡入与窄屏、画廊过渡/自转/滚动到底/返回。

模型、地形、材质、相机、轮组动画、命中爆炸消散、布局和黑场衔接与基线一致；29 组中 25 组 RGB 像素完全相同。其余为画廊 4 张，视觉未见布局或模型变化；未将这些图宣称为逐像素一致。

`report-comparison.json`：七份报告归一输出路径、斜杠，以及菜单诊断中的实体 ID 与查询顺序后全部一致；原始报告完整保留。`pixel-comparison.json` 保存逐图结果。

`assets-before.json` 与 `assets-after.json` 对所有 16 个已跟踪资产 SHA-256 比较相同。

## 评审与边界

按 code-review 分 Standards / Spec 两个独立评审：规范初评发现 WIDTH/HEIGHT/Target 的可见性偏宽，已改为私有；规格评审未发现行为变化或缺失。最终复核结果见本机评审记录。

未运行 Python 或模型生成器（调用链未变）；既有 Python SPAWN_PAD_M 基线失败未修复。未测试 Linux、其他显卡或真实窗口输入；未启动可见窗口。

Cargo 有既有增量缓存硬链接失败后回退复制警告，未阻断。菜单/画廊仍有基线已记录的 ICU4X 分词日志，未在本任务调查。
