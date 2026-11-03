//! 炮镜相机与视角槽位切换：常驻 3D 相机严格跟随炮管世界位姿，Shift 单按互换主副槽。
//!
//! 炮镜位姿为纯函数（炮管 GlobalTransform → 相机 Transform），安装点为炮口端面
//! （沿炮轴 −5.42 m 常量），投影 zoom ×1（FOV 同第三人称默认垂直 45°），far 2000 m
//! 对齐弹丸射程。视角状态为单一枚举资源（第三人称主 / 炮镜主），Shift 按下互换两相机的
//! 槽位参数（全屏/矩形、优先级），瞬时完成，无过渡动画；长按/连按只响应首次按下。

use crate::gameplay::camera::FollowCamera;
use crate::gameplay::combat::firing::PlayerBarrel;
use crate::tank::model::astra;
use bevy::{
    camera::Viewport,
    prelude::*,
    transform::TransformSystems,
};

/// 标记炮镜 3D 相机，位姿每帧严格取炮管世界位姿。
#[derive(Component)]
pub(crate) struct GunnerCamera;

/// 视角槽位状态：哪台相机占据主槽（全屏）。
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum ViewMode {
    /// 第三人称主槽（默认），炮镜副槽（右上矩形）。
    #[default]
    ThirdPersonMain,
    /// 炮镜主槽（全屏），第三人称副槽（右上矩形）。
    GunnerMain,
}

/// 主槽（全屏）相机渲染优先级：先于副槽渲染，为副槽叠加让位。
pub(crate) const MAIN_ORDER: isize = -2;
/// 副槽（矩形视口）相机渲染优先级：晚于主槽、先于 chrome UI 相机（0），
/// 独立清屏后把副画面叠加到主画面之上。
pub(crate) const SUB_ORDER: isize = -1;

/// 炮镜相机位姿纯函数：炮管世界位姿 → 相机 Transform。
///
/// 安装点为炮口端面（沿炮轴 −5.42 m），相机前向（−Z）对齐炮管前向（−X），
/// 因此旋转需额外绕 Y 轴转 90°。
pub(crate) fn gunner_camera_transform(barrel: &GlobalTransform) -> Transform {
    let muzzle = Vec3::from_array(astra::MUZZLE_POSITION);
    let position = barrel.transform_point(muzzle);
    let rotation = barrel.rotation() * Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    Transform::from_translation(position).with_rotation(rotation)
}

/// 副槽视口占位：设计像素 320×180、距窗口右/上缘 24px，乘 UiScale 与 DPR 由 03 提供。
///
/// 当前为最小实现：直接使用物理像素，不乘 UiScale/DPR；窗口 resize 重算由 03 提供。
pub(crate) fn pip_viewport(window_physical: UVec2) -> Viewport {
    let width = 320.min(window_physical.x.saturating_sub(24));
    let height = 180.min(window_physical.y.saturating_sub(24));
    let x = window_physical.x.saturating_sub(width + 24);
    let y = 24;
    Viewport {
        physical_position: UVec2::new(x, y),
        physical_size: UVec2::new(width, height),
        ..default()
    }
}

/// 注册炮镜相机生成、位姿同步与视角切换系统。
pub(crate) struct GunnerCameraPlugin;

impl Plugin for GunnerCameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewMode>()
            .add_systems(
                PostUpdate,
                sync_gunner_camera.after(TransformSystems::Propagate),
            )
            .add_systems(Update, switch_view);
    }
}

/// 每帧同步炮镜相机位姿：严格取炮管世界位姿，安装点炮口端面。
///
/// 炮管场景实例未就绪时静默跳过，不污染上一帧可用姿态。
fn sync_gunner_camera(
    barrels: Query<&GlobalTransform, With<PlayerBarrel>>,
    mut cameras: Query<&mut Transform, With<GunnerCamera>>,
) {
    let Ok(barrel) = barrels.single() else {
        return;
    };
    let transform = gunner_camera_transform(barrel);
    if !transform.translation.is_finite() || !transform.rotation.is_finite() {
        return;
    }
    for mut camera in &mut cameras {
        *camera = transform;
    }
}

/// 按槽位写相机参数：主槽全屏（viewport 清空），副槽矩形视口。
fn apply_slot(camera: &mut Camera, main: bool, pip: &Viewport) {
    if main {
        camera.viewport = None;
        camera.order = MAIN_ORDER;
    } else {
        camera.viewport = Some(pip.clone());
        camera.order = SUB_ORDER;
    }
}

/// Shift 单按互换主副槽位：长按/连按只响应首次按下，瞬时完成无过渡。
///
/// 切换后 TankAim 数值不变（视角状态不触碰炮塔/炮管角）。
fn switch_view(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<ViewMode>,
    windows: Query<&Window>,
    mut follow: Query<&mut Camera, (With<FollowCamera>, Without<GunnerCamera>)>,
    mut gunner: Query<&mut Camera, (With<GunnerCamera>, Without<FollowCamera>)>,
) {
    if !(keys.just_pressed(KeyCode::ShiftLeft) || keys.just_pressed(KeyCode::ShiftRight)) {
        return;
    }
    *mode = match *mode {
        ViewMode::ThirdPersonMain => ViewMode::GunnerMain,
        ViewMode::GunnerMain => ViewMode::ThirdPersonMain,
    };
    let window_physical = windows
        .single()
        .map(|w| w.resolution.physical_size())
        .unwrap_or(UVec2::new(1280, 720));
    let pip = pip_viewport(window_physical);
    let third_person_main = matches!(*mode, ViewMode::ThirdPersonMain);
    for mut camera in &mut follow {
        apply_slot(&mut camera, third_person_main, &pip);
    }
    for mut camera in &mut gunner {
        apply_slot(&mut camera, !third_person_main, &pip);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 任意炮塔 yaw / 炮管 pitch 下，炮镜位姿 = 炮管世界位姿（安装点炮口端面、前向炮管 −X）。
    #[test]
    fn gunner_camera_pose_matches_barrel_world_pose() {
        let barrel = GlobalTransform::IDENTITY;
        let pose = gunner_camera_transform(&barrel);
        assert!(
            pose.translation.abs_diff_eq(Vec3::new(-5.42, 0.0, 0.0), 1e-5),
            "安装点应为炮口端面：{:?}",
            pose.translation
        );
        assert!(
            pose.forward().as_vec3().abs_diff_eq(Vec3::NEG_X, 1e-5),
            "前向应对齐炮管 −X：{:?}",
            pose.forward().as_vec3()
        );

        let yaw = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let pitch = Quat::from_rotation_z(-20.0_f32.to_radians());
        let barrel = GlobalTransform::from(Transform::from_rotation(yaw * pitch));
        let pose = gunner_camera_transform(&barrel);
        let expected_pos =
            yaw * pitch * Vec3::from_array(astra::MUZZLE_POSITION);
        assert!(
            pose.translation.abs_diff_eq(expected_pos, 1e-4),
            "yaw 90° pitch 20° 位置应为 {:?}，实际 {:?}",
            expected_pos,
            pose.translation
        );
        let expected_fwd = (yaw * pitch * Vec3::NEG_X).normalize();
        assert!(
            pose.forward().as_vec3().abs_diff_eq(expected_fwd, 1e-4),
            "yaw 90° pitch 20° 前向应为 {:?}，实际 {:?}",
            expected_fwd,
            pose.forward().as_vec3()
        );
    }

    /// 副槽视口占位：1280×720 窗口下应为右上 320×180、距右/上缘 24px。
    #[test]
    fn pip_viewport_placeholder_anchors_top_right() {
        let viewport = pip_viewport(UVec2::new(1280, 720));
        assert_eq!(viewport.physical_position, UVec2::new(936, 24));
        assert_eq!(viewport.physical_size, UVec2::new(320, 180));
    }

    /// 窗口小于占位尺寸时，视口应钳制到可用区域，不溢出。
    #[test]
    fn pip_viewport_clamps_to_small_window() {
        let viewport = pip_viewport(UVec2::new(400, 200));
        assert_eq!(viewport.physical_size, UVec2::new(320, 176));
        assert_eq!(viewport.physical_position, UVec2::new(56, 24));
    }

    /// 同步系统：炮管 GlobalTransform 变化后，炮镜相机位姿当帧跟随。
    #[test]
    fn sync_system_updates_camera_from_barrel_global_transform() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin))
            .add_systems(PostUpdate, sync_gunner_camera.after(TransformSystems::Propagate));
        let barrel = app
            .world_mut()
            .spawn((
                PlayerBarrel,
                Transform::from_rotation(
                    Quat::from_rotation_y(0.5) * Quat::from_rotation_z(-0.3),
                ),
            ))
            .id();
        let camera = app
            .world_mut()
            .spawn((GunnerCamera, Transform::default()))
            .id();
        app.update();
        let barrel_global = app.world().get::<GlobalTransform>(barrel).unwrap();
        let expected = gunner_camera_transform(barrel_global);
        let actual = app.world().get::<Transform>(camera).unwrap();
        assert!(
            actual.translation.abs_diff_eq(expected.translation, 1e-4),
            "位置应为 {:?}，实际 {:?}",
            expected.translation,
            actual.translation
        );
        assert!(
            actual.rotation.abs_diff_eq(expected.rotation, 1e-4),
            "旋转应为 {:?}，实际 {:?}",
            expected.rotation,
            actual.rotation
        );
    }

    /// Shift 单按互换槽位、repeat 不触发；切换后 TankAim 数值不变。
    #[test]
    fn shift_swaps_view_slots_and_repeat_does_not_retrigger() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ViewMode>()
            .add_systems(Update, switch_view);
        let follow = app
            .world_mut()
            .spawn((
                FollowCamera,
                Camera {
                    order: -2,
                    ..default()
                },
            ))
            .id();
        let gunner = app
            .world_mut()
            .spawn((
                GunnerCamera,
                Camera {
                    order: -1,
                    ..default()
                },
            ))
            .id();
        // 视角切换不得触碰炮塔/炮管角度；此实体作为回归哨兵。
        let tank = app
            .world_mut()
            .spawn((
                crate::gameplay::player::Tank,
                crate::tank::articulation::TankAim {
                    yaw: 0.3,
                    pitch: 0.1,
                },
            ))
            .id();
        assert_eq!(
            *app.world().resource::<ViewMode>(),
            ViewMode::ThirdPersonMain
        );
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::ShiftLeft);
        app.update();
        assert_eq!(
            *app.world().resource::<ViewMode>(),
            ViewMode::GunnerMain,
            "首次按下应切换为炮镜主"
        );
        let follow_camera = app.world().get::<Camera>(follow).unwrap();
        let gunner_camera = app.world().get::<Camera>(gunner).unwrap();
        assert!(follow_camera.viewport.is_some(), "第三人称应变副槽");
        assert!(gunner_camera.viewport.is_none(), "炮镜应为主槽");
        assert_eq!(follow_camera.order, -1);
        assert_eq!(gunner_camera.order, -2);
        // 长按不连发：模拟帧末清理（真实应用由 keyboard_input_system 执行 clear）
        // 后 OS repeat 重发同一键；已按住的键不会再进入 just_pressed，不应再次切换。
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.clear();
            keys.press(KeyCode::ShiftLeft);
        }
        app.update();
        assert_eq!(
            *app.world().resource::<ViewMode>(),
            ViewMode::GunnerMain,
            "长按/连按不应重复切换"
        );
        // 松开后按 ShiftRight 应切回。
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.clear();
        keys.release(KeyCode::ShiftLeft);
        keys.press(KeyCode::ShiftRight);
        app.update();
        assert_eq!(
            *app.world().resource::<ViewMode>(),
            ViewMode::ThirdPersonMain,
            "ShiftRight 应切回第三人称主"
        );
        let follow_camera = app.world().get::<Camera>(follow).unwrap();
        let gunner_camera = app.world().get::<Camera>(gunner).unwrap();
        assert!(follow_camera.viewport.is_none(), "第三人称应恢复主槽");
        assert!(gunner_camera.viewport.is_some(), "炮镜应变副槽");
        assert_eq!(follow_camera.order, -2);
        assert_eq!(gunner_camera.order, -1);
        let aim = app
            .world()
            .get::<crate::tank::articulation::TankAim>(tank)
            .unwrap();
        assert_eq!((aim.yaw, aim.pitch), (0.3, 0.1), "切换视角前后炮塔/炮管角度必须保持不动");
    }
}
