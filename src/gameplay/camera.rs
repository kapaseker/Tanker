//! 对局跟随相机：在本帧变换传播前同步车体与炮塔姿态。

use crate::{gameplay::player::Tank, tank::articulation::TankAim};
use bevy::{prelude::*, transform::TransformSystems};

/// 相机距车身中心的水平后退距离与世界竖直高度，单位：米。
const CAMERA_DISTANCE: f32 = 16.0;
const CAMERA_HEIGHT: f32 = 9.0;
/// 相对于看向车身中心的方向向上抬起的角度，单位：度。
const CAMERA_PITCH_UP_DEGREES: f32 = 8.0;

/// 标记跟随炮塔水平朝向的游戏相机，不接受鼠标控制。
#[derive(Component)]
pub(crate) struct FollowCamera;

/// 注册渲染帧相机跟随系统。
pub(crate) struct FollowCameraPlugin;

impl Plugin for FollowCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, follow_tank.before(TransformSystems::Propagate));
    }
}

/// 模型局部 +X 是车尾；yaw 是炮塔相对车体的偏航角（弧度）。
pub(crate) fn camera_transform(tank: &Transform, yaw: f32) -> Transform {
    let rear = tank.rotation * Quat::from_rotation_y(yaw) * Vec3::X;
    let horizontal = Vec3::new(rear.x, 0.0, rear.z).normalize_or_zero();
    let position = tank.translation + horizontal * CAMERA_DISTANCE + Vec3::Y * CAMERA_HEIGHT;
    let mut camera = Transform::from_translation(position).looking_at(tank.translation, Vec3::Y);
    camera.rotate_local_x(CAMERA_PITCH_UP_DEGREES.to_radians());
    camera
}

/// 坦克与相机均为根节点，在本帧变换传播前同步机位，避免画面滞后一帧。
fn follow_tank(
    tanks: Query<(&Transform, &TankAim), With<Tank>>,
    mut cameras: Query<&mut Transform, (With<FollowCamera>, Without<Tank>)>,
) {
    let Ok((tank, aim)) = tanks.single() else {
        return;
    };
    for mut camera in &mut cameras {
        *camera = camera_transform(tank, aim.yaw);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 已知方向下相机的位置与炮管俯仰无关。
    #[test]
    fn camera_follows_turret_cardinal_directions() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin))
            .add_systems(PostUpdate, follow_tank.before(TransformSystems::Propagate));
        let tank = app
            .world_mut()
            .spawn((
                Tank,
                Transform::default(),
                crate::tank::articulation::TankAim::default(),
            ))
            .id();
        let camera = app
            .world_mut()
            .spawn((FollowCamera, Transform::default()))
            .id();
        for (yaw, expected) in [
            (0.0_f32, Vec3::new(16.0, 9.0, 0.0)),
            (90.0, Vec3::new(0.0, 9.0, -16.0)),
            (-90.0, Vec3::new(0.0, 9.0, 16.0)),
            (180.0, Vec3::new(-16.0, 9.0, 0.0)),
            (360.0, Vec3::new(16.0, 9.0, 0.0)),
        ] {
            let mut aim = app
                .world_mut()
                .get_mut::<crate::tank::articulation::TankAim>(tank)
                .unwrap();
            aim.yaw = yaw.to_radians();
            aim.pitch = 0.2;
            app.update();
            assert!(
                app.world()
                    .get::<Transform>(camera)
                    .unwrap()
                    .translation
                    .abs_diff_eq(expected, 0.001)
            );
        }
    }

    /// 俯仰、回绕及车体倾斜不改变规定的世界机位约束。
    #[test]
    fn camera_preserves_height_pitch_and_wrap() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin)).add_systems(PostUpdate, follow_tank);
        let tank = app.world_mut().spawn((Tank, Transform::from_rotation(Quat::from_rotation_z(0.3)), TankAim::default())).id();
        let camera = app.world_mut().spawn((FollowCamera, Transform::default())).id();
        app.update();
        let first = *app.world().get::<Transform>(camera).unwrap();
        assert!((first.translation.y - 9.0).abs() < 0.001);
        assert!((first.translation.xz().length() - 16.0).abs() < 0.001);
        { let mut aim = app.world_mut().get_mut::<TankAim>(tank).unwrap(); aim.pitch = 0.3; aim.yaw = std::f32::consts::TAU; }
        app.update();
        let next = app.world().get::<Transform>(camera).unwrap();
        assert!(next.translation.abs_diff_eq(first.translation, 0.001));
        assert!(next.rotation.abs_diff_eq(first.rotation, 0.0001));
    }

    /// 相机当帧跟随平移与转向，视线保持向上抬 8°；倒车不把机位切到车头。
    #[test]
    fn camera_follows_position_heading_and_reverse_without_frame_delay() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin))
            .add_systems(
                PostUpdate,
                follow_tank.before(bevy::transform::TransformSystems::Propagate),
            );
        let tank = app
            .world_mut()
            .spawn((Tank, Transform::default(), TankAim::default()))
            .id();
        let camera = app
            .world_mut()
            .spawn((FollowCamera, Transform::default()))
            .id();
        for (position, heading, expected_offset) in [
            (Vec3::ZERO, Quat::IDENTITY, Vec3::new(16.0, 9.0, 0.0)),
            (
                Vec3::new(-6.0, 0.75, 4.0),
                Quat::IDENTITY,
                Vec3::new(16.0, 9.0, 0.0),
            ),
            (
                Vec3::new(-6.0, 0.75, 4.0),
                Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
                Vec3::new(0.0, 9.0, -16.0),
            ),
            (
                Vec3::new(-6.0, 0.75, 0.0),
                Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
                Vec3::new(0.0, 9.0, -16.0),
            ),
        ] {
            *app.world_mut().get_mut::<Transform>(tank).unwrap() =
                Transform::from_translation(position).with_rotation(heading);
            app.update();
            let view = app.world().get::<Transform>(camera).unwrap();
            assert!(
                view.translation
                    .abs_diff_eq(position + expected_offset, 1e-5)
            );
            let forward = view.forward().as_vec3();
            let original_pitch = -expected_offset.y.atan2(expected_offset.xz().length());
            assert!((forward.y.asin() - original_pitch - 8.0_f32.to_radians()).abs() < 1e-5);
            assert!(
                forward
                    .xz()
                    .normalize()
                    .abs_diff_eq(-expected_offset.xz().normalize(), 1e-5)
            );
            let global = app.world().get::<GlobalTransform>(camera).unwrap();
            assert!(global.translation().abs_diff_eq(view.translation, 1e-5));
        }
    }
}
