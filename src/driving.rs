//! 方向键街机驾驶与轨道相机位置跟随；物理单位为米、秒。

use crate::firing::Tank;
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraSystemSet};

/// 当前 tank.vox 的包围盒尺寸（X 长、Y 高、Z 宽），单位：米。
// shortcut: 按现有模型标定；更换 tank.vox 时同步更新包围盒与炮口。
pub(super) const TANK_SIZE: Vec3 = Vec3::new(1.6, 0.75, 1.2);
/// 前进与后退速度，单位：米/秒。
const FORWARD_SPEED: f32 = 3.0;
const REVERSE_SPEED: f32 = 2.0;
/// 左右转向速度，单位：弧度/秒（90°/秒）。
const TURN_SPEED: f32 = std::f32::consts::FRAC_PI_2;

/// 注册固定物理步驾驶及渲染帧相机跟随。
pub(super) struct DrivingPlugin;

impl Plugin for DrivingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedPostUpdate,
            drive_tank
                .after(PhysicsSystems::Prepare)
                .before(PhysicsSystems::StepSimulation),
        )
        .add_systems(PostUpdate, follow_tank.before(PanOrbitCameraSystemSet));
    }
}

/// 简化刚体承托整个模型，锁住俯仰和侧倾；取消摩擦以保持街机匀速。
pub(super) fn tank_body() -> impl Bundle {
    (
        RigidBody::Dynamic,
        Collider::cuboid(TANK_SIZE.x, TANK_SIZE.y, TANK_SIZE.z),
        LockedAxes::new().lock_rotation_x().lock_rotation_z(),
        Friction::ZERO,
        SleepingDisabled,
    )
}

/// 根据车头方向写入水平速度，保留重力及接触求解产生的竖直速度。
fn apply_input(keys: &ButtonInput<KeyCode>, heading: Quat, linear: &mut Vec3, angular: &mut Vec3) {
    let speed = match (
        keys.pressed(KeyCode::ArrowUp),
        keys.pressed(KeyCode::ArrowDown),
    ) {
        (true, false) => FORWARD_SPEED,
        (false, true) => -REVERSE_SPEED,
        _ => 0.0,
    };
    let forward = heading * Vec3::NEG_X;
    linear.x = forward.x * speed;
    linear.z = forward.z * speed;
    // +Y 为车头左转；倒车不反转转向，相反方向同时按下时抵消。
    let turn =
        i32::from(keys.pressed(KeyCode::ArrowLeft)) - i32::from(keys.pressed(KeyCode::ArrowRight));
    *angular = Vec3::Y * turn as f32 * TURN_SPEED;
}

/// 在物理准备完成后读取权威姿态，避免渲染变换同步造成方向滞后一帧。
fn drive_tank(
    keys: Res<ButtonInput<KeyCode>>,
    mut tanks: Query<(&Rotation, &mut LinearVelocity, &mut AngularVelocity), With<Tank>>,
) {
    for (rotation, mut linear, mut angular) in &mut tanks {
        apply_input(&keys, rotation.0, &mut linear.0, &mut angular.0);
    }
}

/// 坦克根节点没有父实体，直接读取本帧 Transform；相机方位和距离仍由鼠标控制。
fn follow_tank(tanks: Query<&Transform, With<Tank>>, mut cameras: Query<&mut PanOrbitCamera>) {
    let Ok(tank) = tanks.single() else {
        return;
    };
    for mut camera in &mut cameras {
        camera.target_focus = tank.translation;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证倒车速度、冲突按键抵消、松键即停及竖直速度保留。
    #[test]
    fn arrows_control_speed_and_cancel_opposing_inputs() {
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::ArrowDown);
        let mut linear = Vec3::new(1.0, -0.4, 1.0);
        let mut angular = Vec3::ONE;
        apply_input(&keys, Quat::IDENTITY, &mut linear, &mut angular);
        assert!(linear.abs_diff_eq(Vec3::new(2.0, -0.4, 0.0), 1e-5));
        keys.press(KeyCode::ArrowUp);
        keys.press(KeyCode::ArrowLeft);
        keys.press(KeyCode::ArrowRight);
        apply_input(&keys, Quat::IDENTITY, &mut linear, &mut angular);
        assert_eq!(linear, Vec3::new(0.0, -0.4, 0.0));
        assert_eq!(angular, Vec3::ZERO);
        keys.clear();
        apply_input(&keys, Quat::IDENTITY, &mut linear, &mut angular);
        assert_eq!(linear, Vec3::new(0.0, -0.4, 0.0));
        assert_eq!(angular, Vec3::ZERO);
    }

    /// 左转是世界 +Y 角速度；车头局部 −X 旋转后沿世界 +Z 前进。
    #[test]
    fn movement_follows_heading_and_turning_does_not_reverse_when_backing() {
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::ArrowUp);
        keys.press(KeyCode::ArrowLeft);
        let mut linear = Vec3::ZERO;
        let mut angular = Vec3::ZERO;
        let heading = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        apply_input(&keys, heading, &mut linear, &mut angular);
        assert!(linear.abs_diff_eq(Vec3::Z * 3.0, 1e-5));
        assert!(angular.abs_diff_eq(Vec3::Y * std::f32::consts::FRAC_PI_2, 1e-5));
        keys.release(KeyCode::ArrowUp);
        keys.press(KeyCode::ArrowDown);
        apply_input(&keys, heading, &mut linear, &mut angular);
        assert!(linear.abs_diff_eq(Vec3::NEG_Z * 2.0, 1e-5));
        assert!(angular.y > 0.0);
        keys.release(KeyCode::ArrowLeft);
        keys.press(KeyCode::ArrowRight);
        apply_input(&keys, heading, &mut linear, &mut angular);
        assert!(angular.y < 0.0);
    }

    /// 无窗口运行真实 Avian：落地、行驶、松键、转向及边界碰撞。
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
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(step))
        .add_systems(Startup, |mut commands: Commands| {
            crate::ground::spawn_arena(&mut commands)
        });
        // 手动逐帧测试也需完成 Avian 在 finish 阶段注册的求解器资源。
        app.finish();
        app.cleanup();
        let tank = app
            .world_mut()
            .spawn((Tank, tank_body(), Transform::from_xyz(0.0, 2.0, 0.0)))
            .id();
        for _ in 0..192 {
            app.update();
        }
        let position = app.world().get::<Transform>(tank).unwrap().translation;
        assert!(
            (position.y - TANK_SIZE.y / 2.0).abs() < 0.02,
            "落地高度异常：{position:?}"
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowUp);
        for _ in 0..64 {
            app.update();
        }
        assert!((app.world().get::<Transform>(tank).unwrap().translation.x + 3.0).abs() < 0.05);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::ArrowUp);
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
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowLeft);
        for _ in 0..64 {
            app.update();
        }
        let heading = app.world().get::<Transform>(tank).unwrap().rotation;
        assert!((heading * Vec3::NEG_X).abs_diff_eq(Vec3::Z, 0.03));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::ArrowLeft);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowUp);
        for _ in 0..640 {
            app.update();
        }
        let transform = app.world().get::<Transform>(tank).unwrap();
        let forward = transform.rotation * Vec3::NEG_X;
        let side = transform.rotation * Vec3::Z;
        let extent_z = forward.z.abs() * TANK_SIZE.x / 2.0 + side.z.abs() * TANK_SIZE.z / 2.0;
        assert!(transform.translation.z > 14.0);
        assert!(transform.translation.z + extent_z <= 1000.0_f32.sqrt() / 2.0 + 0.03);
        assert!((transform.rotation * Vec3::Y).abs_diff_eq(Vec3::Y, 1e-4));
    }
}
