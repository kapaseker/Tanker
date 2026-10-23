//! 车尾跟随相机：标记固定机位，并在本帧变换传播前同步玩家位姿。

use crate::gameplay::player::Tank;
use bevy::{prelude::*, transform::TransformSystems};

/// 相机距车身中心的水平后退距离与世界竖直高度，单位：米。
const CAMERA_DISTANCE: f32 = 16.0;
const CAMERA_HEIGHT: f32 = 9.0;
/// 相对于看向车身中心的方向向上抬起的角度，单位：度。
const CAMERA_PITCH_UP_DEGREES: f32 = 8.0;

/// 标记固定在车尾上方的游戏相机，不接受鼠标控制。
#[derive(Component)]
pub(crate) struct FollowCamera;

/// 注册渲染帧相机跟随系统。
pub(crate) struct FollowCameraPlugin;

impl Plugin for FollowCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, follow_tank.before(TransformSystems::Propagate));
    }
}

/// 模型局部 +X 是车尾；从看向车身中心的方向向上抬视线，机位保持不变。
pub(crate) fn camera_transform(tank: &Transform) -> Transform {
    let position =
        tank.translation + tank.rotation * Vec3::X * CAMERA_DISTANCE + Vec3::Y * CAMERA_HEIGHT;
    let mut camera = Transform::from_translation(position).looking_at(tank.translation, Vec3::Y);
    camera.rotate_local_x(CAMERA_PITCH_UP_DEGREES.to_radians());
    camera
}

/// 坦克与相机均为根节点，在本帧变换传播前同步机位，避免画面滞后一帧。
fn follow_tank(
    tanks: Query<&Transform, With<Tank>>,
    mut cameras: Query<&mut Transform, (With<FollowCamera>, Without<Tank>)>,
) {
    let Ok(tank) = tanks.single() else {
        return;
    };
    for mut camera in &mut cameras {
        *camera = camera_transform(tank);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 相机当帧跟随平移与转向，视线保持向上抬 8°；倒车不把机位切到车头。
    #[test]
    fn camera_follows_position_heading_and_reverse_without_frame_delay() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin))
            .add_systems(
                PostUpdate,
                follow_tank.before(bevy::transform::TransformSystems::Propagate),
            );
        let tank = app.world_mut().spawn((Tank, Transform::default())).id();
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
