# 06: 敌方车身和炮管迁移到 0.02 米

Status: done
Type: task
Blocked by: 05

## What to build

暗红敌方车身和炮管完成同色体积分解，继续与旧敌方履带及已细分玩家同场显示。

上下文：先读 [规格](../spec.md) 和 [执行方案](../plan.md)，以及阻塞任务的结果与证据。状态仅表示计划可执行，本轮没有开始实施。

## Acceptance criteria

- [x] 敌方 body/barrel 配置为 0.02、各有 125 倍体素；颜色、材质、装甲形状、炮口与语义节点不变。
- [x] 敌方非炮管米制尺寸保持 6.6×2.6×3.4、完整视觉长 9 米；生成净空、落地与碰撞关系不变。
- [x] Python 原生成器及测试走同一个转换发布流程，生成结果可重现；不修改字面量契约解析为执行任意 Rust/Python 表达式。

## Verification

- [x] cargo check；敌方生成/装配/碰撞/预览的相关 Rust 与 Python 测试。
- [x] scene、gallery、detail 关键帧确认同场比例、炮管装配和材质不变；用户明确取消敌方 wheels 测试（证据见评论区 2026-10-29 硬件 GPU 记录）。
- [x] 检查玩家资产没有回退，非坦克资产哈希保持；保存粗源与目标的体积、位置和尺寸对照。

## 代码定位（实施前重新核对）

scripts/generate_enemy.py；敌方 body/barrel/meta；scripts/test_generate_enemy.py；src/tank/model/enemy.rs 或相关米制断言。

## 边界

不新增敌方开火或子弹。维持 ADR 中已有部件路径、型号与边界。

## Comments

2026-10-28：用户明确本次仅细分，不做美术精修。完成后在此记录命令、结果、证据和遗留项；通过前不把任务记为完成。

2026-10-28（实施中，未验收）：车身从 19,772 细分为 2,471,500，炮管从 268 细分为 33,500；单位均为 0.02。粗源参考格占用边界（半开区间）：车身 X 25..90、Y 1..33、Z 5..26；炮管 X 76..114、Y 15..19、Z 18..22。转换器逐格核对 125 倍有色体积；生成入口 `--check` 逐字节比对。玩家、敌方履带及非坦克资产相对实施前 Git 版本无改动。

`cargo check`、Python 全套 42 项通过；真实装配测试通过，画廊缺失资产测试在全量运行时两次超时、单独重跑通过。scene 和 gallery 离屏通过，画廊报告敌方整车 9.0×3.4×2.6 米；详情部分图片已生成，但 llvmpipe 下 120 秒超时，未通过。敌方 wheels 退出 1：帧 30/60 前后直行里程近零，旋转阶段断言通过。证据分别在 `.scratch/vox-06-scene/`、`.scratch/vox-06-gallery/`、`.scratch/vox-06-detail/`、`.scratch/vox-06-enemy-wheels/`，不可将失败算作通过。需排查 wheels 直行失败并补全详情验收；暂不标记完成。

补充：以 HEAD 的旧车身/炮管及 0.1 meta 构造独立资源根，使用相同 wheels 命令复现完全相同的帧 30/60 失败，报告见 `.scratch/vox-06-baseline-enemy-wheels/`。原因是 `ENEMY_TRACKS.forward_speed` 和 `reverse_speed` 均为 0 的既有占位值，测试仍要求直行；与细分无关。再次低负载单线程全量 Rust 测试仍为 103 通过、画廊缺失资源等待超时 1 项；单项测试通过。

2026-10-29：用户明确取消敌方 wheels 测试，敌方速度保持 0。恢复临时诊断代码后 `cargo check` 与 `cargo test -- --test-threads=1` 完整通过 113 项，证据 `.scratch/vox-06-clean-suite-3.log`。先前画廊和履带测试存在间歇加载超时，画廊超时诊断显示缺失资产仍处于 Loading，根因未确认。

详情离屏按原 120 秒上限重试多次，默认、优化编译及 llvmpipe 4/6/8/16 线程均未通过；`.scratch/vox-06-detail-retry2/` 和 `retry3/` 已保存九种型号、交互与缺失资源关键帧，但最终报告超时，不能视为完整通过。当前环境没有 `/dev/dri`，无法使用硬件 GPU。用户指定取消轮组测试不等于取消详情验收，因此工单仍待详情全流程通过；用户要求先提交现有进度。

2026-10-29（续接）：实现与当前证据先提交，工单保持未完成。后续在能访问 Vulkan 硬件 GPU 的环境，从工程根目录执行 `cargo run -- --offscreen-test detail --output .scratch/vox-06-detail-gpu`（若目录已存在则使用新目录），检查退出码、报告和敌方车身/炮管/整车关键帧。通过后记录证据并完成工单；若仍失败，保留失败报告继续排查。敌方 wheels 测试已按用户要求取消。

2026-10-29（完成）：在 Windows + NVIDIA RTX 4060（报告记为 Vulkan 后端）硬件环境完成详情验收。

- 先修复详情间歇失败的真实竞态：`src/pages/model_preview.rs` 的 `prepare_previews` 原先注册在 Update，而场景实例在 Update 与 PostUpdate 之间的 SpawnScene 才生成，确定性晚到的网格会带着默认层进入验收，报“模型网格渲染层不匹配”。将 `prepare_previews`（画廊连同 `spin`、详情连同 `spawn_axes`）改到 Last 调度，同帧补层；断言未放宽。改动前复现一次失败（`.scratch/vox-06-detail-gpu-06/report.txt`，目录项 6），改动后连续三次通过。
- `cargo check` 通过；`cargo test -- --test-threads=1` 104+5+4=113 项全部通过。
- detail：连续三次 `exit 0`、报告“结果：通过”，证据 `.scratch/vox-06-detail-gpu-005143/`、`vox-06-detail-gpu-r005848-1/`、`vox-06-detail-gpu-r005959-2/`（27 个产物/次）。报告尺寸：敌方车身 6.5×3.2×2.1、炮管 3.8×0.4×0.4、整车 9.0×3.4×2.6 米，可见相位 2、轮 12，与粗源米制契约一致。
- scene：`exit 0`、结果通过，`.scratch/vox-06-scene-gpu-005359/`；frame-000000 与 legacy-scene-02 同场比例一致，玩家绿色坦克、混凝土墩与 HUD 无变化。
- gallery：`exit 0`、结果通过，`.scratch/vox-06-gallery-gpu-005408/`；gallery-top 中 lance/spit 炮管材质与子弹伴随位置不变，其余卡片为已知资产比例。
- 关键帧目视对照：`.scratch/vox-06-detail-gpu-005143/detail-7-initial.png` 与工单 01 基准 `.scratch/vox-model-refinement/evidence/legacy-detail-01/detail-7-initial.png` 的敌方整车外轮廓、颜色分区、炮管装配位置一致；detail-5（车身）、detail-1（炮管）同样一致，未见分块裂缝、色差或内部面异常。
- 环境说明：此前 Linux llvmpipe 的 120 秒超时已不再复现，硬件 GPU 下 detail 全流程约 60 秒。敌方 wheels 仍按用户要求不测（速度为 0 的既有占位，不属本工单）。
