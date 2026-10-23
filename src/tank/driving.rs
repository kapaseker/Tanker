//! 逐车街机驾驶；物理单位为米、秒。
//!
//! 只消费 [`DrivingInput`]，不识别玩家身份、键盘、页面状态或敌人：
//! 玩家键盘写入意图见 `gameplay::player`，车尾相机跟随见 `gameplay::camera`。

use avian3d::prelude::*;
use bevy::prelude::*;

/// 玩家车身与履带的单一碰撞框（X 长、Y 高、Z 宽），不包含伸出的炮管，单位米。
pub(crate) const TANK_SIZE: Vec3 = Vec3::new(
    (crate::tank::model::player::COLLISION_MAX[0] - crate::tank::model::player::COLLISION_MIN[0]) as f32
        * crate::tank::model::VOXEL_SIZE,
    (crate::tank::model::player::COLLISION_MAX[2] - crate::tank::model::player::COLLISION_MIN[2]) as f32
        * crate::tank::model::VOXEL_SIZE,
    (crate::tank::model::player::COLLISION_MAX[1] - crate::tank::model::player::COLLISION_MIN[1]) as f32
        * crate::tank::model::VOXEL_SIZE,
);
/// 前进与后退速度，单位：米/秒。
const FORWARD_SPEED: f32 = 6.0;
const REVERSE_SPEED: f32 = 4.0;
/// 左右转向速度，单位：弧度/秒（90°/秒）。
const TURN_SPEED: f32 = std::f32::consts::FRAC_PI_2;

/// 转向平滑时间，单位：秒。数值越小响应越硬，越大越"绵"
const TURN_SMOOTH_TIME: f32 = 0.15;

/// 每辆坦克各自的驾驶意图；敌人默认零输入，未来控制器可直接写入。
#[derive(Component, Default)]
pub(crate) struct DrivingInput {
    /// 前后输入，范围 −1..=1；正值前进，负值倒车，零立即停止水平驱动。
    pub(crate) throttle: f32,
    /// 左右输入，范围 −1..=1；正值左转，倒车不反转转向。
    pub(crate) turn: f32,
}

/// 每辆坦克当前的转向角速度，单位：弧度/秒；带符号，+Y 为左转。
#[derive(Component, Default)]
pub(crate) struct TurnRate(pub(crate) f32);

/// 注册固定物理步驾驶系统。
pub(crate) struct DrivingPlugin;

impl Plugin for DrivingPlugin {
    fn build(&self, app: &mut App) {
        // 长度翻倍、时间尺度不变，重力也翻倍以保持落地耗时。
        app.insert_resource(Gravity(Gravity::default().0 * 2.0)).add_systems(
            FixedPostUpdate,
            drive_tank
                .after(PhysicsSystems::Prepare)
                .before(PhysicsSystems::StepSimulation),
        );
    }
}

/// 简化刚体承托整个模型，锁住全部旋转（俯仰、侧倾、偏航）。
/// 偏航由驾驶系统直接积分写入，物理接触（起伏地形、碰撞）无法扭转车头，
/// 避免体素台阶的接触冲量把直线行驶扭成蛇形。
pub(crate) fn tank_body() -> impl Bundle {
    body_for_size(TANK_SIZE)
}

/// 坦克碰撞体的圆角半径，单位：米。
///
/// 体素地形碰撞体是垂直台阶（cuboid 侧面），纯方盒撞任何坎都会被水平法线
/// 挡死。圆角大于一档坎高（0.1 米）时，坎顶边缘低于圆心，接触法线带向上
/// 分量，坦克借行驶推力翻上矮坎；两档及以上的高坎仍是墙。圆角通过
/// "核心盒收短 2×半径 + 圆角半径"合成，碰撞体总尺寸与 [`TANK_SIZE`] 等车身
/// 尺寸保持一致。
const STEP_CLIMB_RADIUS: f32 = 0.2;

/// 按整车尺寸创建共享驾驶刚体与逐车状态；尺寸单位为米，敌人不接玩家输入。
pub(crate) fn body_for_size(size: Vec3) -> impl Bundle {
    (
        DrivingInput::default(),
        TurnRate::default(),
        RigidBody::Dynamic,
        // 圆角箱形碰撞体：外形不变、四角收圆，让坦克能翻越一档体素台阶。
        Collider::round_cuboid(
            size.x - 2.0 * STEP_CLIMB_RADIUS,
            size.y - 2.0 * STEP_CLIMB_RADIUS,
            size.z - 2.0 * STEP_CLIMB_RADIUS,
            STEP_CLIMB_RADIUS,
        ),
        LockedAxes::new()
            .lock_rotation_x()
            .lock_rotation_y()
            .lock_rotation_z(),
        Friction::ZERO,
        SleepingDisabled,
    )
}

/// 根据车头方向写入水平速度，保留重力及接触求解产生的竖直速度；
/// 车头朝向由本函数独占：转向角速度按固定步长 dt 向目标（±TURN_SPEED 或 0）
/// 指数渐近后直接积分进 heading，绕世界 Y 旋转。物理不参与偏航。
fn apply_input(
    input: &DrivingInput,
    dt: f32,
    current_turn: &mut f32,
    linear: &mut Vec3,
    heading: &mut Quat,
) {
    let throttle = input.throttle.clamp(-1.0, 1.0);
    let speed = throttle
        * if throttle >= 0.0 {
            FORWARD_SPEED
        } else {
            REVERSE_SPEED
        };
    let forward = *heading * Vec3::NEG_X;
    linear.x = forward.x * speed;
    linear.z = forward.z * speed;
    // +Y 为车头左转；倒车不反转转向，相反方向同时按下时抵消。
    let target = input.turn.clamp(-1.0, 1.0) * TURN_SPEED;
    // 帧率无关的指数渐近：按住时逼近目标角速度，松键时渐降到 0。
    let smooth = 1.0 - (-dt / TURN_SMOOTH_TIME).exp();
    *current_turn += (target - *current_turn) * smooth;
    // 世界系 Y 轴左转积分；车体只含偏航，与局部 Y 等价。
    *heading = Quat::from_axis_angle(Vec3::Y, *current_turn * dt) * *heading;
}

/// 在物理准备完成后逐车应用输入，避免共享转向状态或渲染姿态滞后一帧。
/// `pub(crate)` 仅供 `gameplay::player` 的输入系统声明"输入先于驾驶"的调度边。
pub(crate) fn drive_tank(
    time: Res<Time>,
    mut tanks: Query<(
        &DrivingInput,
        &mut TurnRate,
        &mut Rotation,
        &mut LinearVelocity,
    )>,
) {
    for (input, mut turn_rate, mut rotation, mut linear) in &mut tanks {
        apply_input(
            input,
            time.delta_secs(),
            &mut turn_rate.0,
            &mut linear.0,
            &mut rotation.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真实圆角碰撞体尺寸与偏航包围盒；炮管不得扩展刚体。
    #[test]
    fn player_collider_matches_geometry_contract_when_rotated() {
        let mut world = World::new();
        let tank = world.spawn(tank_body()).id();
        let collider = world.get::<Collider>(tank).unwrap();
        assert!(TANK_SIZE.abs_diff_eq(Vec3::new(7.5, 2.6, 3.6), 1e-5));
        for yaw in [
            0.0,
            std::f32::consts::FRAC_PI_4,
            std::f32::consts::FRAC_PI_2,
        ] {
            let rotation = Quat::from_rotation_y(yaw);
            let position = Vec3::new(2.0, 1.3, -3.0);
            // 几何尺寸契约不包含物理宽相位额外扩张的边距。
            let actual = collider.aabb(position, rotation, 0.0);
            let core = TANK_SIZE / 2.0 - Vec3::splat(STEP_CLIMB_RADIUS);
            let expected = (rotation * Vec3::X * core.x).abs()
                + Vec3::Y * core.y
                + (rotation * Vec3::Z * core.z).abs()
                + Vec3::splat(STEP_CLIMB_RADIUS);
            assert!(actual.min.abs_diff_eq(position - expected, 1e-5));
            assert!(actual.max.abs_diff_eq(position + expected, 1e-5));
        }
    }

    /// 验证倒车速度、零输入立即停止、竖直速度保留及无转向输入不改变朝向。
    /// 键盘到意图的映射由 `gameplay::player` 的映射测试覆盖。
    #[test]
    fn inputs_control_speed_and_zero_input_stops() {
        const DT: f32 = 1.0 / 64.0;
        let mut current_turn = 0.0_f32;
        let mut linear = Vec3::new(1.0, -0.4, 1.0);
        let mut heading = Quat::IDENTITY;
        apply_input(
            &DrivingInput {
                throttle: -1.0,
                turn: 0.0,
            },
            DT,
            &mut current_turn,
            &mut linear,
            &mut heading,
        );
        assert!(linear.abs_diff_eq(Vec3::new(4.0, -0.4, 0.0), 1e-5));
        assert!(heading.abs_diff_eq(Quat::IDENTITY, 1e-5));
        // 方向相反的输入抵消后为零，立即停止水平驱动。
        apply_input(
            &DrivingInput {
                throttle: 0.0,
                turn: 0.0,
            },
            DT,
            &mut current_turn,
            &mut linear,
            &mut heading,
        );
        assert_eq!(linear, Vec3::new(0.0, -0.4, 0.0));
        assert!(heading.abs_diff_eq(Quat::IDENTITY, 1e-5));
        // 持续零输入保持停止。
        apply_input(
            &DrivingInput {
                throttle: 0.0,
                turn: 0.0,
            },
            DT,
            &mut current_turn,
            &mut linear,
            &mut heading,
        );
        assert_eq!(linear, Vec3::new(0.0, -0.4, 0.0));
        assert!(heading.abs_diff_eq(Quat::IDENTITY, 1e-5));
    }

    /// 左转沿世界 +Y 积分偏航；车头局部 −X 旋转后沿世界 +Z 前进；
    /// 倒车不反转转向；换向后的角速度渐降，数步后偏航才回落。
    // 转向平滑后角速度逐步逼近目标，故只断言偏航的增减方向而非精确值。
    #[test]
    fn movement_follows_heading_and_turning_does_not_reverse_when_backing() {
        const DT: f32 = 1.0 / 64.0;
        let mut current_turn = 0.0_f32;
        let mut linear = Vec3::ZERO;
        let mut heading = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        apply_input(
            &DrivingInput {
                throttle: 1.0,
                turn: 1.0,
            },
            DT,
            &mut current_turn,
            &mut linear,
            &mut heading,
        );
        assert!(linear.abs_diff_eq(Vec3::Z * 6.0, 1e-5));
        assert!(yaw(heading) > std::f32::consts::FRAC_PI_2);
        // 速度取积分前的朝向（本步先算速度、后积分偏航），车头略微越过 +Z。
        let forward = heading * Vec3::NEG_X;
        apply_input(
            &DrivingInput {
                throttle: -1.0,
                turn: 1.0,
            },
            DT,
            &mut current_turn,
            &mut linear,
            &mut heading,
        );
        assert!(linear.abs_diff_eq(forward * -REVERSE_SPEED, 1e-5));
        assert!(linear.abs_diff_eq(Vec3::NEG_Z * REVERSE_SPEED, 0.03));
        // 倒车不反转转向：角速度仍为正，偏航继续增大。
        let left_yaw = yaw(heading);
        // 单步渐变后角速度仍为正，需数步才越过 0 转为负，偏航随之回落。
        for _ in 0..4 {
            apply_input(
                &DrivingInput {
                    throttle: -1.0,
                    turn: -1.0,
                },
                DT,
                &mut current_turn,
                &mut linear,
                &mut heading,
            );
        }
        assert!(yaw(heading) < left_yaw);
    }

    /// 提取纯偏航四元数的偏航角，单位：弧度（车体只含 Y 旋转，值域 (−π, π]）。
    fn yaw(heading: Quat) -> f32 {
        2.0 * heading.y.atan2(heading.w)
    }

    /// 无窗口运行真实 Avian：落地、行驶、停车、转向及边界碰撞。
    /// 直接写入驾驶意图而非按键：键盘到意图的映射属玩家控制器职责。
    #[test]
    fn physics_supports_driving_stopping_and_arena_boundaries() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let step = Duration::from_secs_f64(1.0 / 64.0);
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            PhysicsPlugins::default(),
            DrivingPlugin,
        ))
        .insert_resource(TimeUpdateStrategy::ManualDuration(step))
        .add_systems(Startup, |mut commands: Commands| {
            crate::terrain::spawn_arena(&mut commands)
        });
        // 手动逐帧测试也需完成 Avian 在 finish 阶段注册的求解器资源。
        app.finish();
        app.cleanup();
        let tank = app
            .world_mut()
            .spawn((tank_body(), Transform::from_xyz(0.0, 4.0, 0.0)))
            .id();
        for _ in 0..192 {
            app.update();
        }
        assert_eq!(
            app.world().resource::<Gravity>().0,
            Vec3::new(0.0, -19.62, 0.0)
        );
        let position = app.world().get::<Transform>(tank).unwrap().translation;
        assert!(
            (position.y - TANK_SIZE.y / 2.0).abs() < 0.02,
            "落地高度异常：{position:?}"
        );
        app.world_mut().get_mut::<DrivingInput>(tank).unwrap().throttle = 1.0;
        for _ in 0..64 {
            app.update();
        }
        assert!((app.world().get::<Transform>(tank).unwrap().translation.x + 6.0).abs() < 0.05);
        app.world_mut().get_mut::<DrivingInput>(tank).unwrap().throttle = 0.0;
        let stopped = app.world().get::<Transform>(tank).unwrap().translation;
        for _ in 0..32 {
            app.update();
        }
        assert!(
            app.world()
                .get::<Transform>(tank)
                .unwrap()
                .translation
                .abs_diff_eq(stopped, 0.01)
        );
        // 倒车一秒应后退 4 米，再清零输入验证下一段转向不夹带直线速度。
        app.world_mut().get_mut::<DrivingInput>(tank).unwrap().throttle = -1.0;
        for _ in 0..64 {
            app.update();
        }
        let reversed = app.world().get::<Transform>(tank).unwrap().translation;
        assert!((reversed.x - stopped.x - 4.0).abs() < 0.05);
        app.world_mut().get_mut::<DrivingInput>(tank).unwrap().throttle = 0.0;
        app.world_mut().get_mut::<DrivingInput>(tank).unwrap().turn = 1.0;
        for _ in 0..64 {
            app.update();
        }
        // 转向平滑后 1 秒按住的累计转角略小于 90°，用指数渐近的解析积分算期望值：
        // θ = TURN_SPEED × (T − TURN_SMOOTH_TIME × (1 − e^(−T/TURN_SMOOTH_TIME)))。
        let t = 1.0_f32;
        let theta = TURN_SPEED * (t - TURN_SMOOTH_TIME * (1.0 - (-t / TURN_SMOOTH_TIME).exp()));
        let heading = app.world().get::<Transform>(tank).unwrap().rotation;
        // 车头局部 −X 绕 +Y 左转 θ 后指向 (−cosθ, 0, sinθ)。
        let expected = Vec3::new(-theta.cos(), 0.0, theta.sin());
        assert!(
            (heading * Vec3::NEG_X).abs_diff_eq(expected, 0.03),
            "朝向偏差异常：{heading:?} 期望 {expected:?}"
        );
        app.world_mut().get_mut::<DrivingInput>(tank).unwrap().turn = 0.0;
        // 直行直到抵住 300 米处的墙：6 米/秒 × 53 秒 ≈ 318 米，足够贴墙并保持压力。
        app.world_mut().get_mut::<DrivingInput>(tank).unwrap().throttle = 1.0;
        for _ in 0..3400 {
            app.update();
        }
        let transform = app.world().get::<Transform>(tank).unwrap();
        let forward = transform.rotation * Vec3::NEG_X;
        let side = transform.rotation * Vec3::Z;
        let extent_z = forward.z.abs() * TANK_SIZE.x / 2.0 + side.z.abs() * TANK_SIZE.z / 2.0;
        let wall = crate::terrain::FIELD_SIDE / 2.0;
        let stop = transform.translation.z + extent_z;
        assert!((stop - wall).abs() < 0.03, "未到达墙：{transform:?}");
        assert!(stop <= wall + 0.03, "穿墙：stop={stop} wall={wall}");
        assert!((transform.rotation * Vec3::Y).abs_diff_eq(Vec3::Y, 1e-4));
    }
}
