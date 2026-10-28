# 02: 全部旧 VOX 接入单位配置且保持画面

Status: done
Type: task
Blocked by: 01

## What to build

九个现有 VOX 各有明确的 0.1 米配置，对局、画廊、详情和测试全部按配置加载，旧模型字节与实际表现不变。

上下文：先读 [规格](../spec.md) 和 [执行方案](../plan.md)，以及阻塞任务的结果与证据。状态仅表示计划可执行，本轮没有开始实施。

## Acceptance criteria

- [x] 九份 VOX 均有合法单位配置；取消生产和测试中的 global_settings 单位覆盖，保留原材质/偏移/网格数据设置；模型 meta 随资产分发。
- [x] 九个旧 VOX 哈希与 01 完全相同，米制尺寸、原点、炮口、轮位、轮组运动和爆炸现有缩放不变；不修改 UI 设计。
- [x] 受管理资产缺配置或非法配置时不能静默以插件默认 1 米加载；测试覆盖完整性、路径和可诊断失败。

## Verification

- [x] cargo check；运行真实资产加载/预览/发射/轮组相关测试。
- [x] 使用 01 相同环境和固定帧执行 scene、gallery、detail、wheels；比较图像并检查报告和退出码。
- [x] 核对全部 VOX SHA-256 以及配置清单；通过后记录阶段 A 检查点。

## 代码定位（实施前重新核对）

assets/ 下九份 .vox 的伴随 meta；src/app/mod.rs；gallery/track/explosion/firing 测试组装；资产完整性接缝。

## 边界

不写五倍细分资产；此提交必须能单独回退并证明配置接入零视觉影响。

## Comments

2026-10-28：用户明确本次仅细分，不做美术精修。完成后在此记录命令、结果、证据和遗留项；通过前不把任务记为完成。

2026-10-28（完成）：见 [02-legacy-config.md](../evidence/02-legacy-config.md)。九份 `.vox.meta`（0.1 米/格，其余字段与 01 冻结样例一致）随资产分发；生产与 4 处测试组装取消 global_settings 覆盖；新增 `src/app/vox_meta.rs` 启动完整性守卫（缺配置/损坏/非法 loader/非正单位均 panic 带路径）+ 7 项单测。全量 `cargo test` 109 项通过；九个 VOX SHA-256 与 01 基准 9/9 一致；离屏 scene/wheels 全帧、gallery 17–18/21 帧与 baseline 逐字节一致（差异为 UI 过渡固有时间非确定性，同代码重跑差异集合不重叠）；detail 首次运行通过且米制标注正常；敌方 wheels 与 01 基准同因失败（ENEMY_TRACKS 0.0 占位），非回归。阶段 A 检查点成立。遗留：证据目录下残留两份"幻影"重复运行产物（legacy-scene-02、legacy-gallery-02，实为同代码重跑对照样本，已用于非确定性归因）；执行环境存在双执行痕迹与一次 rustc 编译期崩溃（0xc0000005），均非游戏问题。
