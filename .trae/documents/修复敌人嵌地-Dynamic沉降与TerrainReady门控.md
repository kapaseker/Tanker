# 修复敌人坦克嵌进地形：Static 改 Dynamic + 落体沉降

## 摘要

- **问题**：`enemy.rs` 用 `RigidBody::Static` 把敌人放在固定 Y=0.7 米处——这是"平地场地"时代的假设。e8c00b8 引入 ±1 米起伏高度场后，±120 米范围内的出生点处敌人最深被埋 1 米（车高 1.4 米的 71%）或悬空 1 米，静态刚体不会被物理推正。
- **修复**：生产路径改用 **`Dynamic` 刚体 + 从 3 米高处落下沉降**（物理引擎标准做法，不采样地形高度）；用 `TerrainReady` 资源把敌人生成推迟到地形碰撞体就绪之后，消除"碰撞体未加载时先下坠"的竞态。
- **Static 保留为测试专用**（固定靶语义），只存在于 `#[cfg(test)]`，生产代码不再出现。

## 现状分析

- [enemy.rs L63-L86](file:///e:/Workspace/Code/Rust/Tanker/src/enemy.rs#L63-L86)：`spawn_enemies` 注册在 `Startup`，生成 `RigidBody::Static + Collider::cuboid(ENEMY_SIZE) + Friction::ZERO`，Y 恒为 `ENEMY_SIZE.y / 2.0 = 0.7`；注释"敌人是落在 Y=0 平面上的固定障碍物"已过时。
- [enemy.rs L191](file:///e:/Workspace/Code/Rust/Tanker/src/enemy.rs#L191)：`random_spawns_respect_clearance_and_rotated_bounds` 断言 `translation.y == ENEMY_SIZE.y / 2.0`，需随新出生高度更新。
- [enemy.rs L117-L179](file:///e:/Workspace/Code/Rust/Tanker/src/enemy.rs#L117-L179)：`static_enemy_blocks_driving_until_removed` 自建 Static 实体，不经过 `spawn_enemies`，**不需要改**。
- [driving.rs L44-L52](file:///e:/Workspace/Code/Rust/Tanker/src/driving.rs#L44-L52)：玩家 `tank_body()` = Dynamic + TANK_SIZE 碰撞体 + 锁 X/Z 旋转 + **Friction::ZERO + SleepingDisabled**。不能直接复用：① 敌人模型是 `enemy_tank.vox`（34×26×14 格 = 3.4×1.4×2.6 米），碰撞体应用 `ENEMY_SIZE`；② `Friction::ZERO` 对玩家是刻意设计（每帧覆写水平速度防滑），敌人没有速度控制器，零摩擦会在地形坡面上持续下滑——敌人需要**默认摩擦**站住；③ 敌人应允许休眠（省 CPU 且静止更稳），不带 `SleepingDisabled`。
- [driving.rs L32](file:///e:/Workspace/Code/Rust/Tanker/src/driving.rs#L32)：全局重力 ×2（19.62 m/s²），落体很快。
- [ground.rs](file:///e:/Workspace/Code/Rust/Tanker/src/ground.rs)：地形图异步加载，`spawn_terrain`（Update 轮询）在两图就绪后编译网格并挂高度场碰撞体——**Startup 时碰撞体不存在**，这是敌人生成必须推迟的硬约束。
- [firing.rs L159-L191](file:///e:/Workspace/Code/Rust/Tanker/src/firing.rs#L159-L191)：子弹按 `With<EnemyTank>` 筛选、命中后 despawn 双方——对 Dynamic 敌人同样有效，无需改动。

## 变更清单

### 1. `src/ground.rs` — 就绪信号

在 `spawn_terrain` 编译完成后插入资源：

```rust
/// 标记地形网格与高度场碰撞体已铺设完成，供依赖地形的生成逻辑（敌人）等待。
#[derive(Resource)]
pub(crate) struct TerrainReady;
```

`spawn_terrain` 的首次编译分支末尾 `commands.insert_resource(TerrainReady);`。

### 2. `src/enemy.rs` — 核心修复

1. **生成时机**：`spawn_enemies` 从 `Startup` 挪到 `Update`，签名加 `ready: Option<Res<TerrainReady>>` 与 `Local<bool>` 单次守卫；`TerrainReady` 不存在时直接返回。
2. **刚体**：新增 `enemy_body()` bundle（放在 `spawn_enemies` 上方）：

```rust
/// 敌人刚体：动态落地沉降到地形表面；与玩家 `tank_body()` 的差异均有理由：
/// 碰撞体按敌人模型标定（ENEMY_SIZE）；保留默认摩擦以在坡面站住（玩家零摩擦靠
/// 每帧覆写速度，敌人无速度控制器）；允许休眠保持静止省开销。
fn enemy_body() -> impl Bundle {
    (
        RigidBody::Dynamic,
        Collider::cuboid(ENEMY_SIZE.x, ENEMY_SIZE.y, ENEMY_SIZE.z),
        LockedAxes::new().lock_rotation_x().lock_rotation_z(),
        Friction::default(),
    )
}
```

3. **出生高度**：新增常量并替换 `ENEMY_SIZE.y / 2.0`：

```rust
/// 出生下落起点高度，单位：米；高于全场最高地形（+1.0）加半车高（0.7），
/// 落体由物理引擎沉降到地形表面，无需采样高度图。
const SPAWN_DROP_Y: f32 = 3.0;
```

4. **注释更新**：模块文档"静止敌人靶车"→ 说明物理沉降；L63 过时注释改为"敌人从高处落下，由重力沉降到地形表面；测试用 Static 固定靶见 tests"。

### 3. `src/enemy.rs` 测试更新

- `random_spawns_respect_clearance_and_rotated_bounds`：`assert_eq!(transform.translation.y, ENEMY_SIZE.y / 2.0)` → `assert_eq!(transform.translation.y, SPAWN_DROP_Y)`。
- 其余测试不动（`static_enemy_blocks_driving_until_removed` 自建 Static 实体，语义正是"测试专用固定靶"）。

### 4. 不改的部分

- `firing.rs`：命中 despawn 逻辑对 Dynamic 同样有效。
- 玩家 `tank_body()`：不抽公共抽象——两边刚体配置各有刻意差异（见现状分析），强行合并反而要加参数。

## 假设与决定

| # | 决定 | 理由 |
|---|---|---|
| D1 | 落体沉降而非采样高度图定 Y | 无需新 ground API、无需同步高度图数据；物理求解器本就负责放置动态物体 |
| D2 | `TerrainReady` 资源门控敌人生成 | 地形图异步加载，Startup 时碰撞体不存在；从 2 米处下坠若无碰撞体会穿透场地 |
| D3 | 不复用 `tank_body()` | 碰撞体尺寸、摩擦、休眠三处差异均有物理理由（见上） |
| D4 | 敌人保持无 AI、静止站桩 | 本期只修物理放置；AI 驾驶是后续工作 |
| D5 | 玩家可推动敌人 | Dynamic 的自然结果，属真实感预期，非回归 |

## 验证步骤

1. 工程根目录 `cargo check` 通过。
2. `cargo test -j 2` 全绿（Windows 下 rustc 偶发并行编译崩溃，必要时降并行重试）。
3. `cargo run` 实机检查：日志出现 `地形地图加载完成` 与 `已随机放置 10 辆敌人坦克`；画面上 10 辆敌人**全部立于地形表面**，无嵌入、无悬空（重点看远处起伏区）；开炮击毁一辆确认 despawn 正常。
4. 开车撞一辆敌人：应能推动（Dynamic 预期行为）。
