# 07: 敌方履带相位和轮组迁移到 0.02 米

Status: done
Type: task
Blocked by: 06

## What to build

敌方履带包的六个相位与十二个轮子完成细分，保持原运动和击毁清理行为。

上下文：先读 [规格](../spec.md) 和 [执行方案](../plan.md)，以及阻塞任务的结果与证据。状态仅表示计划可执行，本轮没有开始实施。

## Acceptance criteria

- [x] 包内有色体积精确细分，单位为 0.02；每侧四个负重轮和两个端轮语义、轴心及半径不变。
- [x] 沿用 05 的全子树包围盒与显隐处理，三相位物理移动距离、轮角、停车稳定及净空约束保持。
- [x] 敌方击毁时整棵分块场景及轮组实例全部清理；替换旧实例数量断言时仍证明无残留，不降低清理标准。

## Verification

- [x] cargo check；cargo test tank::track；敌方资源、履带与清理测试，Python 敌方/履带/转换器测试。
- [x] 离屏 wheels --model enemy、drive、combat，并读关键帧与报告（敌人 wheels 直行/倒车里程断言受静态敌人速度 0 占位限制，预期失败且与基线一致；其余断言通过，见 Comments）。
- [x] 比较 01 敌方轮组图和同场比例，检查分块边界、前后端轮旋转及击毁后的实例计数归零。

## 代码定位（实施前重新核对）

scripts/generate_enemy.py 的履带输出；敌方 track.vox/meta；履带/战斗/敌方资源测试；必要的语义清理断言。

## 边界

不把分块 mesh 数变多当成漏清理，也不忽略真实残留；测量整个语义子树。

## Comments

2026-10-28：用户明确本次仅细分，不做美术精修。完成后在此记录命令、结果、证据和遗留项；通过前不把任务记为完成。

2026-10-29 完成：`generate_enemy.py`改为与玩家同一条细分发布链，三包粗样经`subdivide_vox.py`细分到0.02后原子发布，`--check`逐字节比对通过；`track.vox` 43 KB→4.9 MB。
验证：`cargo check`通过；串行`cargo test -- --test-threads=1` 113项（104单元、5+4集成）全通过；Python 42项、`publish_player.py --check`、玩家生成器4项全通过；`real_vox_assets_fire_and_clean_up_scene_instances`断言20个场景根实例随根实体销毁全部注销并逐个确认分块子实体不存在。
离屏（RTX 4060/Vulkan）：drive、combat退出0且报告通过；`wheels --model enemy`仅帧30/60直行里程断言失败，失败行与`legacy-wheels-enemy-01`完全相同，根因是既有的`ENEMY_TRACKS`速度0占位（敌人不移动），属工单06范围外的遗留；转向、停车及轮组其余断言通过。
画面：同10张关键图与01基线对比，帧0/75/90/250及side零差异，其余差异像素为18/379/306/338/236（总像素921600）、最大通道差29、无>32像素，属光栅取样差异；已读侧视、转向与combat第120帧，无分块裂缝、端轮错位或残留。证据：`.scratch/vox-model-refinement/evidence/07-{drive-d86752,combat-92c312,enemy-wheels-a}/`。
附带修整：`src/pages/gallery/mod.rs`缺失资产等待由5秒放宽到10秒——细分后资产变大，串行跑测实测6.23秒才收到失败回报，5秒上限会误报卡死；删除`scripts/test_tracks.py`中已无调用者的旧`read_models`。
遗留：敌方直行/倒车里程与启动速度仍为0，接入敌方AI时另开工单恢复；若要求敌方 wheels 直行断言也必须通过，需先给静态敌人增加受控位移的验证路径。
