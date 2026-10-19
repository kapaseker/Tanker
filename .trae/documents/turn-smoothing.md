# 坦克转向平滑（方案 2：角速度渐近）

## Summary

为驾驶系统增加转向平滑：当前角速度不再随按键瞬间跳变，而是逐物理步指数渐近目标角速度（±`TURN_SPEED` 或 0）。点按产生小角度细腻修正，按住则平滑达到最大转向，松键平滑停转。线性速度（前进/后退）保持瞬时，不做平滑。

## Current State Analysis

- [driving.rs](file:///e:/Workspace/Code/Rust/Tanker/src/driving.rs) 的 `apply_input`（L55-71）直接把 `turn (-1/0/1) × TURN_SPEED` 写入 `AngularVelocity`，按键即瞬间 90°/s，松键瞬间归零。
- `drive_tank`（L74-81）在 `FixedPostUpdate` 中按固定步长（1/64 s）运行，状态目前全在刚体速度组件里，无额外状态。
- 单玩家坦克驱动假设已存在（`follow_tank` 用 `tanks.single()`；敌军坦克为静态障碍物）。
- 现有 4 个测试中 3 个直接断言 `angular` 精确值，平滑后会受渐变影响，需要同步更新。

## Proposed Changes

仅修改 [src/driving.rs](file:///e:/Workspace/Code/Rust/Tanker/src/driving.rs)，不改其他文件。

### 1. 新增状态与常量

- 新常量：`TURN_SMOOTH_TIME: f32 = 0.1`（秒）。含义：当前角速度与目标差距缩小到约 37% 所需时间；数值越小响应越硬，越大越"绵"。可后续手感和调参。
- 新资源（单玩家坦克，资源最简）：

```rust
/// 玩家坦克当前的转向角速度状态，单位：弧度/秒；带符号，+Y 为左转。
#[derive(Resource, Default)]
struct TurnRate(f32);
```

在 `DrivingPlugin::build` 中 `insert_resource(TurnRate::default())`。

> ponytail: 用全局资源而非组件，因为当前只有一辆受控坦克；若未来出现多辆动态可驾驶坦克，应把 `TurnRate` 改为挂在 Tank 实体上的组件。

### 2. 修改 `apply_input`（L55-71）

签名增加 `dt: f32` 与 `current_turn: &mut f32`：

```rust
// +Y 为车头左转；倒车不反转转向，相反方向同时按下时抵消。
let target = i32::from(keys.pressed(KeyCode::ArrowLeft)) as f32
    - i32::from(keys.pressed(KeyCode::ArrowRight)) as f32;
let target = target * TURN_SPEED;
// 帧率无关的指数渐近：按住时逼近目标角速度，松键时渐降到 0。
let smooth = 1.0 - (-dt / TURN_SMOOTH_TIME).exp();
*current_turn += (target - *current_turn) * smooth;
*angular = Vec3::Y * *current_turn;
```

线性速度部分不变。

### 3. 修改 `drive_tank`（L74-81）

- 增加参数 `time: Res<Time>`（FixedPostUpdate 中 `delta_secs()` 即固定步长 1/64 s）与 `mut turn_rate: ResMut<TurnRate>`。
- 调用处传 `time.delta_secs()` 与 `&mut turn_rate.0`。

## Assumptions & Decisions

- **`TURN_SPEED` 保留**：角色从"瞬时角速度"变为平滑渐近的目标上限（回答用户问题）。
- 平滑只作用于转向，前进/后退保持瞬时启停（街机匀速设计不变）。
- 状态放资源而非组件：当前仅一辆受控坦克，改动最小。
- `TURN_SMOOTH_TIME = 0.1s`：约 3 步内爬升到 ~37%，1 秒按住累计转角约 81°（略低于 90°），点按一步约 0.2°，比原来 ~1.4° 更细腻。

## Verification

1. `cargo check` 通过。
2. `cargo test` 通过，需同步更新 3 处受影响的断言：
   - `arrows_control_speed_and_cancel_opposing_inputs`（L165）：`current_turn` 从 0 起且目标为 0，`angular == ZERO` 断言仍成立；签名适配即可。
   - `movement_follows_heading_and_turning_does_not_reverse_when_backing`（L186）：首步 `angular` 不再精确等于 `Vec3::Y * FRAC_PI_2`，改为断言 `angular.y > 0.0`（后续 `>0/<0` 断言按渐变值核算仍成立）。
   - `physics_supports_driving_stopping_and_arena_boundaries`（L280-284）：按住左转 1 秒后朝向不再是精确 90°，改用公式计算期望角 `θ = TURN_SPEED × (T − TURN_SMOOTH_TIME × (1 − e^(−T/TURN_SMOOTH_TIME)))`，断言 `heading` 旋转角与 `θ` 一致（容差 ~0.02 rad）。
3. `cargo run` 手感验证：轻点左右键坦克小幅缓转；按住约 0.3 秒达到全速转向；松键平滑回正无瞬间急停。
