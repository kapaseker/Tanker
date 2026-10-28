# 01: 冻结基准并验证模型配置先于加载生效

Status: ready-for-agent
Type: task
Blocked by: None

## What to build

给现有加载方式建立可信基准，再用真实加载器证明每文件单位配置能作用于网格、命名节点及场景平移。此项交付技术结论和可重复小样，不替换正式资产。

上下文：先读 [规格](../spec.md) 和 [执行方案](../plan.md)，以及阻塞任务的结果与证据。状态仅表示计划可执行，本轮没有开始实施。

## Acceptance criteria

- [x] 重新记录当前 Git 状态、九个 VOX 的哈希/字节/体素数，留存实际场景、画廊和轮组基准；不重置已有修改。
- [x] 0.1 米的 10 格物体和 0.02 米的 50 格物体同场均为 1 米；整场景、#命名节点、非零节点平移均按各自 meta 烘焙；测试不得用根节点缩放作弊。
- [x] 检查原生 meta 的配置优先级、默认值和全部必要设置；对缺失、损坏、非有限或非正单位记录可诊断策略；原生路线若失败，保留最小复现并修订计划，不能直接宣称支持。

## Verification

- [x] 使用锁定版本真实 AssetServer/VoxScenePlugin 的无 GPU 集成小样，检查变换后的米制顶点/AABB。
- [x] Rust 有修改就运行 cargo check；只跑本项测试，基准离屏按 plan.md 的相应场景执行并读取关键帧。
- [x] 证据记录插件源码位置、实际生效的完整 meta 示例、运行环境、退出码和对照数值。

## 代码定位（实施前重新核对）

src/app/mod.rs；真实资产测试的既有组装入口；新增小样/集成测试；本功能 evidence/。

## 边界

01 只验证原生资产元数据，不引入自定义加载器、运行时缩放方案或依赖升级。

## Comments

2026-10-28：用户明确本次仅细分，不做美术精修。完成后在此记录命令、结果、证据和遗留项；通过前不把任务记为完成。

2026-10-28（实施完成，待提交确认）：
- 命令与结果：`cargo check` 通过；`cargo test --test vox_meta_settings` 5/5 通过（exit=0）；离屏 `scene`/`gallery`/`wheels`（玩家）基准全部"结果：通过"（exit=0），关键帧已人工读取确认画面。
- 证据：`.scratch/vox-model-refinement/evidence/01-loading-proof.md`（含对照数值表、可诊断策略结论、环境与退出码）、`effective-meta-example.ron`（真实生效 meta 全量示例）、`baseline-git-status.txt`、`baseline-vox-sha256.txt`（九个 VOX 哈希/字节）及各 `baseline-*` 产物目录。
- 技术结论：原生 `.vox.meta` 每文件 `voxel_size` 对网格与 nTRN 平移均生效；`global_settings: Some(..)` 会覆盖 meta，必须保持 None；缺失 meta 静默回退 1.0 米/格，损坏 meta 明确失败可诊断，非正/非有限值无校验——校验责任在转换器。`#命名节点` 子资产 scope 根恒为 `Transform::IDENTITY`，命名节点自身 `_t` 平移须调用方外部放置（生产 `wheel_transform` 即此用法）。
- 遗留项：`baseline-wheels-enemy-01` 失败——`ENEMY_TRACKS.forward_speed/reverse_speed = 0.0` 占位值使油门路径恒为 0（ca48d80 起 `drive_tank` 按每车 `TrackSpec` 读速度；转向路径正常，帧 90/120 通过）。属既有已提交状态、超出 01 边界，保留失败证据；建议接入敌方 AI 时赋真实速度并恢复该基准。
- 测试小样：`tests/vox_meta_settings.rs`（内存资产源 `voxfix://` + 真实 `VoxSceneLoader` 管线，无 GPU）。
