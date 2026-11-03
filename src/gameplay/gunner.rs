//! 炮镜相机与视角槽位切换：常驻 3D 相机严格跟随炮管世界位姿，Shift 单按互换主副槽。
//!
//! 炮镜位姿为纯函数（炮管 GlobalTransform → 相机 Transform），安装点为炮口端面
//! （沿炮轴 −5.42 m 常量），投影 zoom ×1（FOV 同第三人称默认垂直 45°），far 2000 m
//! 对齐弹丸射程。视角状态为单一枚举资源（第三人称主 / 炮镜主），Shift 按下翻转模式，
//! 槽位参数（全屏/矩形视口、优先级）由 [`update_view_slots`] 按模式统一应用，
//! 瞬时完成无过渡动画；长按/连按只响应首次按下。
//! 副槽视口为设计像素 320×180、距窗口右/上缘 24px，乘 UiScale（zoom）与窗口 DPR
//! 换算物理像素，窗口 resize 重算；副窗边框为挂在主槽相机 UI 层的 1px 描边节点。

use crate::app::state::GameState;
use crate::gameplay::camera::FollowCamera;
use crate::gameplay::combat::firing::PlayerBarrel;
use crate::gameplay::session::SessionEntity;
use crate::tank::model::astra;
use crate::ui::resources::{colors, dimens};
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

/// 副槽视口矩形换算纯函数：设计像素（320×180、边距 24）乘 UiScale 与窗口 DPR
/// 得物理像素，锚窗口右上（Bevy 视口坐标以窗口左上为原点、y 向下）。
///
/// 窗口不足以容纳完整副窗时尺寸钳制到可用区域，仍保留右上边距不溢出。
pub(crate) fn pip_viewport(window_physical: UVec2, ui_scale: f32, dpr: f32) -> Viewport {
    let design_px = |value: f32| (value * ui_scale * dpr).round().max(0.0) as u32;
    let margin = design_px(dimens::pip::INSET);
    let width = design_px(dimens::pip::WIDTH).min(window_physical.x.saturating_sub(margin));
    let height = design_px(dimens::pip::HEIGHT).min(window_physical.y.saturating_sub(margin));
    Viewport {
        physical_position: UVec2::new(
            window_physical.x.saturating_sub(width + margin),
            margin,
        ),
        physical_size: UVec2::new(width, height),
        ..default()
    }
}

/// 读取窗口得副窗换算三元组（物理尺寸, UiScale, DPR）：
/// UiScale 取整体等比缩放 zoom（按逻辑宽度），DPR 取窗口缩放因子；
/// 无窗口（离屏回退）按 1280×720、DPR 1 基线。
pub(crate) fn window_layout_params(window: Option<&Window>) -> (UVec2, f32, f32) {
    match window {
        Some(w) => {
            let dpr = w.resolution.scale_factor();
            let physical = w.resolution.physical_size();
            let ui_scale = crate::ui::zoom(physical.x as f32 / dpr.max(f32::EPSILON));
            (physical, ui_scale, dpr)
        }
        None => (bevy::math::uvec2(1280, 720), 1.0, 1.0),
    }
}

/// 从窗口读取实际参数换算副槽矩形；无窗口时回退基线换算。
pub(crate) fn pip_viewport_for_window(window: Option<&Window>) -> Viewport {
    let (physical, ui_scale, dpr) = window_layout_params(window);
    pip_viewport(physical, ui_scale, dpr)
}

/// 副窗边框标记：绝对定位 UI 根节点，按副槽矩形描边，随主槽相机全屏渲染。
#[derive(Component)]
pub(crate) struct PipFrame;

/// 注册炮镜相机生成、位姿同步、视角切换与副窗布局系统。
pub(crate) struct GunnerCameraPlugin;

impl Plugin for GunnerCameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewMode>()
            .add_systems(
                PostUpdate,
                sync_gunner_camera.after(TransformSystems::Propagate),
            )
            .add_systems(Update, update_view_slots)
            .add_systems(OnEnter(GameState::InGame), spawn_pip_frame);
    }
}

/// 进入对局生成副窗边框：1px 薄荷描边，与状态簇底板边框同色
/// （原型 `.view.is-pip` 的 rgba(200,219,177,0.38) 线性等效）。
/// 位置尺寸由 [`update_view_slots`] 按窗口换算逐次刷新，初始取设计值。
fn spawn_pip_frame(mut commands: Commands) {
    commands.spawn((
        PipFrame,
        SessionEntity,
        Node {
            position_type: PositionType::Absolute,
            top: px(dimens::pip::INSET),
            right: px(dimens::pip::INSET),
            width: px(dimens::pip::WIDTH),
            height: px(dimens::pip::HEIGHT),
            border: UiRect::all(px(dimens::pip::BORDER_WIDTH)),
            ..default()
        },
        BorderColor::all(colors::hud::PLATE_BORDER),
    ));
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
/// 仅实际差异时写入，避免每帧标记 Camera 变更触发渲染重建。
/// `Viewport` 未实现 `PartialEq`，按本项目构造仅比较位置与尺寸两字段。
fn apply_slot(camera: &mut Camera, main: bool, pip: &Viewport) {
    let order = if main { MAIN_ORDER } else { SUB_ORDER };
    let viewport_matches = match &camera.viewport {
        None => main,
        Some(current) => {
            !main
                && current.physical_position == pip.physical_position
                && current.physical_size == pip.physical_size
        }
    };
    if !viewport_matches || camera.order != order {
        camera.viewport = if main { None } else { Some(pip.clone()) };
        camera.order = order;
    }
}

/// 视角槽位与副窗布局的唯一写入点。
///
/// Shift 单按翻转 [`ViewMode`]（长按/连按只响应首次按下，瞬时完成无过渡）；
/// 每帧按（模式, 窗口物理尺寸, DPR）计算目标槽位参数与副窗边框，仅在实际差异时
/// 写入，避免触发 Camera/Node 变更检测导致无谓的渲染重建与 UI 重排。
/// 系统无跨帧状态，重入对局新生成的相机/边框同样会被立即校正。
/// 切换视角不触碰炮塔/炮管角（TankAim 数值保持）。
/// 无窗口（离屏回退）按 1280×720、DPR 1 基线换算。
fn update_view_slots(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<ViewMode>,
    windows: Query<&Window>,
    mut follow: Query<&mut Camera, (With<FollowCamera>, Without<GunnerCamera>)>,
    mut gunner: Query<&mut Camera, (With<GunnerCamera>, Without<FollowCamera>)>,
    mut frames: Query<&mut Node, With<PipFrame>>,
) {
    if keys.just_pressed(KeyCode::ShiftLeft) || keys.just_pressed(KeyCode::ShiftRight) {
        *mode = match *mode {
            ViewMode::ThirdPersonMain => ViewMode::GunnerMain,
            ViewMode::GunnerMain => ViewMode::ThirdPersonMain,
        };
    }
    let (physical, ui_scale, dpr) = window_layout_params(windows.single().ok());
    let pip = pip_viewport(physical, ui_scale, dpr);
    let third_person_main = matches!(*mode, ViewMode::ThirdPersonMain);
    for mut camera in &mut follow {
        apply_slot(&mut camera, third_person_main, &pip);
    }
    for mut camera in &mut gunner {
        apply_slot(&mut camera, !third_person_main, &pip);
    }
    // 边框节点为逻辑像素：Bevy UI 内部再乘 DPR，故只乘 UiScale。
    let inset = px(dimens::pip::INSET * ui_scale);
    let width = px(dimens::pip::WIDTH * ui_scale);
    let height = px(dimens::pip::HEIGHT * ui_scale);
    let border = UiRect::all(px(dimens::pip::BORDER_WIDTH * ui_scale));
    for mut node in &mut frames {
        if node.top != inset
            || node.right != inset
            || node.width != width
            || node.height != height
            || node.border != border
        {
            node.top = inset;
            node.right = inset;
            node.width = width;
            node.height = height;
            node.border = border;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::window::WindowResolution;

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

    /// 矩形换算基线：1280×720、UiScale 1、DPR 1 下应为右上 320×180、距右/上缘 24px。
    #[test]
    fn pip_viewport_baseline_anchors_top_right() {
        let viewport = pip_viewport(UVec2::new(1280, 720), 1.0, 1.0);
        assert_eq!(viewport.physical_position, UVec2::new(936, 24));
        assert_eq!(viewport.physical_size, UVec2::new(320, 180));
    }

    /// 设计像素乘 UiScale 与 DPR 得物理像素；锚点距右/上缘的边距同样换算。
    #[test]
    fn pip_viewport_scales_by_uiscale_and_dpr() {
        // UiScale 0.75 × DPR 1：320×180 → 240×135，边距 24 → 18。
        let viewport = pip_viewport(UVec2::new(1280, 720), 0.75, 1.0);
        assert_eq!(viewport.physical_size, UVec2::new(240, 135));
        assert_eq!(viewport.physical_position, UVec2::new(1022, 18));
        // UiScale 1 × DPR 2：320×180 → 640×360，边距 24 → 48。
        let viewport = pip_viewport(UVec2::new(1280, 720), 1.0, 2.0);
        assert_eq!(viewport.physical_size, UVec2::new(640, 360));
        assert_eq!(viewport.physical_position, UVec2::new(592, 48));
        // UiScale 0.75 × DPR 2：320×180 → 480×270，边距 24 → 36。
        let viewport = pip_viewport(UVec2::new(1280, 720), 0.75, 2.0);
        assert_eq!(viewport.physical_size, UVec2::new(480, 270));
        assert_eq!(viewport.physical_position, UVec2::new(764, 36));
    }

    /// 窗口不足以容纳完整副窗时，尺寸钳制到可用区域（保留右上边距），不溢出。
    #[test]
    fn pip_viewport_clamps_to_small_window() {
        let viewport = pip_viewport(UVec2::new(300, 200), 1.0, 1.0);
        assert_eq!(viewport.physical_size, UVec2::new(276, 176));
        assert_eq!(viewport.physical_position, UVec2::new(0, 24));
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
            .add_systems(Update, update_view_slots);
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

    /// resize/DPR 重算：副槽视口矩形与副窗边框随窗口换算，两套坐标系保持对齐。
    #[test]
    fn update_view_slots_recomputes_on_window_change() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ViewMode>()
            .add_systems(Update, update_view_slots);
        let window = app
            .world_mut()
            .spawn(Window {
                resolution: WindowResolution::new(1280, 720),
                ..default()
            })
            .id();
        let follow = app
            .world_mut()
            .spawn((FollowCamera, Camera::default()))
            .id();
        let gunner = app
            .world_mut()
            .spawn((GunnerCamera, Camera::default()))
            .id();
        let frame = app
            .world_mut()
            .spawn((PipFrame, Node::default()))
            .id();
        app.update();
        // 基线 1280×720、DPR 1、UiScale 1：炮镜占副槽 320×180、距右/上缘 24px。
        let pip = app
            .world()
            .get::<Camera>(gunner)
            .unwrap()
            .viewport
            .clone()
            .unwrap();
        assert_eq!(pip.physical_position, UVec2::new(936, 24));
        assert_eq!(pip.physical_size, UVec2::new(320, 180));
        assert_eq!(app.world().get::<Camera>(gunner).unwrap().order, SUB_ORDER);
        assert!(app.world().get::<Camera>(follow).unwrap().viewport.is_none());
        assert_eq!(app.world().get::<Camera>(follow).unwrap().order, MAIN_ORDER);
        let node = app.world().get::<Node>(frame).unwrap();
        assert_eq!(node.top, px(dimens::pip::INSET));
        assert_eq!(node.width, px(dimens::pip::WIDTH));
        assert_eq!(node.border.left, px(dimens::pip::BORDER_WIDTH));
        // resize 到 960×540：UiScale 0.75，视口与边框同步缩小。
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution = WindowResolution::new(960, 540);
        app.update();
        let pip = app.world().get::<Camera>(gunner).unwrap().viewport.clone().unwrap();
        assert_eq!(pip.physical_position, UVec2::new(702, 18));
        assert_eq!(pip.physical_size, UVec2::new(240, 135));
        let node = app.world().get::<Node>(frame).unwrap();
        assert_eq!(node.width, px(240.0));
        assert_eq!(node.border.top, px(0.75));
        // DPR 2（逻辑宽 480 → UiScale 0.375）：物理视口不变，边框逻辑像素减半，
        // 两套坐标系换算后仍对齐同一矩形。
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set_scale_factor_override(Some(2.0));
        app.update();
        let pip = app.world().get::<Camera>(gunner).unwrap().viewport.clone().unwrap();
        assert_eq!(pip.physical_position, UVec2::new(702, 18));
        assert_eq!(pip.physical_size, UVec2::new(240, 135));
        let node = app.world().get::<Node>(frame).unwrap();
        assert_eq!(node.width, px(120.0));
        assert_eq!(node.top, px(9.0));
    }
}
