# 06: 敌方车身和炮管迁移到 0.02 米

Status: ready-for-agent
Type: task
Blocked by: 05

## What to build

暗红敌方车身和炮管完成同色体积分解，继续与旧敌方履带及已细分玩家同场显示。

上下文：先读 [规格](../spec.md) 和 [执行方案](../plan.md)，以及阻塞任务的结果与证据。状态仅表示计划可执行，本轮没有开始实施。

## Acceptance criteria

- [ ] 敌方 body/barrel 配置为 0.02、各有 125 倍体素；颜色、材质、装甲形状、炮口与语义节点不变。
- [ ] 敌方非炮管米制尺寸保持 6.6×2.6×3.4、完整视觉长 9 米；生成净空、落地与碰撞关系不变。
- [ ] Python 原生成器及测试走同一个转换发布流程，生成结果可重现；不修改字面量契约解析为执行任意 Rust/Python 表达式。

## Verification

- [ ] cargo check；敌方生成/装配/碰撞/预览的相关 Rust 与 Python 测试。
- [ ] scene、gallery、detail 及敌方 wheels 相关关键帧确认同场比例、炮管装配和材质不变。
- [ ] 检查玩家资产没有回退，非坦克资产哈希保持；保存粗源与目标的体积、位置和尺寸对照。

## 代码定位（实施前重新核对）

scripts/generate_enemy.py；敌方 body/barrel/meta；scripts/test_generate_enemy.py；src/tank/model/enemy.rs 或相关米制断言。

## 边界

不新增敌方开火或子弹。维持 ADR 中已有部件路径、型号与边界。

## Comments

2026-10-28：用户明确本次仅细分，不做美术精修。完成后在此记录命令、结果、证据和遗留项；通过前不把任务记为完成。
