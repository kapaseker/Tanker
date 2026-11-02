//! 实际射击路径准星：只读当帧预测，在布局前投影，不接收指针。

use crate::{
    app::state::GameState,
    gameplay::{
        camera::FollowCamera,
        combat::firing::{ShotPrediction, predict_shot},
        session::SessionEntity,
    },
    ui::resources::colors,
};
use bevy::{
    camera::CameraUpdateSystems, prelude::*, transform::TransformSystems, ui::FocusPolicy,
    ui::UiSystems,
};

/// 28px 根框以接触点居中；十字与圆独立表达方向和预测命中。
#[derive(Component)]
pub(crate) struct Reticle;

/// 薄荷空心圆，只在本帧预测命中时显示。
#[derive(Component)]
pub(crate) struct HitRing;

/// 画面外边缘箭头根框：整体按指向旋转，只表示调炮方向，不表示命中。
#[derive(Component)]
pub(crate) struct EdgeArrow;

/// 边缘箭头整体（16px 方框）距视口边缘的最小距离，单位：逻辑像素。
pub(crate) const EDGE_MARGIN: f32 = 24.;
/// 边缘箭头根框边长；锚点为中心，臂条在框内摆动不会越出，单位：逻辑像素。
pub(crate) const ARROW_SIZE: f32 = 16.;

/// 对局准星的生成及渲染前更新调度。
pub(super) struct ReticlePlugin;
impl Plugin for ReticlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), setup)
            .add_systems(
                PostUpdate,
                update
                    .after(TransformSystems::Propagate)
                    .after(CameraUpdateSystems)
                    .after(predict_shot)
                    .before(UiSystems::Prepare),
            );
    }
}

/// 对局准星与边缘箭头的生成；两者都随 SessionEntity 统一回收。
fn setup(mut commands: Commands) {
    commands
        .spawn((
            Reticle,
            SessionEntity,
            FocusPolicy::Pass,
            GlobalZIndex(10),
            Node {
                position_type: PositionType::Absolute,
                width: px(28),
                height: px(28),
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                HitRing,
                FocusPolicy::Pass,
                Node {
                    position_type: PositionType::Absolute,
                    width: px(28),
                    height: px(28),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BorderColor::all(colors::PRIMARY),
            ));
            // 描边内端与线条齐平，避免向中心伸入而把透明空隙缩成 2×2。
            for ([x, y, w, h], [ox, oy, ow, oh]) in [
                ([4., 13., 8., 2.], [3., 12., 9., 4.]),
                ([16., 13., 8., 2.], [16., 12., 9., 4.]),
                ([13., 4., 2., 8.], [12., 3., 4., 9.]),
                ([13., 16., 2., 8.], [12., 16., 4., 9.]),
            ] {
                parent
                    .spawn((
                        FocusPolicy::Pass,
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(ox),
                            top: px(oy),
                            width: px(ow),
                            height: px(oh),
                            ..default()
                        },
                        BackgroundColor(colors::ON_PRIMARY),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            FocusPolicy::Pass,
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(x - ox),
                                top: px(y - oy),
                                width: px(w),
                                height: px(h),
                                ..default()
                            },
                            BackgroundColor(colors::ON_SURFACE),
                        ));
                    });
            }
        });
    spawn_edge_arrow(&mut commands);
}

/// 画面外方向的空心折线箭头：两段 45° 臂组成朝 +X 的折线，整体由根节点旋转。
///
/// 根框 16×16，锚点即箭头中心；臂条与十字同样采用外描边套内条的双层结构
///（暖黑描边 + 米白条），旋转围绕节点中心进行，子条随父条一起转动。
fn spawn_edge_arrow(commands: &mut Commands) {
    commands
        .spawn((
            EdgeArrow,
            SessionEntity,
            FocusPolicy::Pass,
            GlobalZIndex(10),
            Node {
                position_type: PositionType::Absolute,
                width: px(ARROW_SIZE),
                height: px(ARROW_SIZE),
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|parent| {
            // 上臂：中点 (8,5)，下臂：中点 (8,11)；条心对齐臂中点后绕中心转 ±45°。
            for (mid_y, angle) in [(5.0_f32, 45.0_f32), (11., -45.)] {
                parent
                    .spawn((
                        FocusPolicy::Pass,
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(8. - 5.),
                            top: px(mid_y - 2.),
                            width: px(10.),
                            height: px(4.),
                            ..default()
                        },
                        // 描边条 10×4 的中心恰为臂中点，旋转不偏移。
                        UiTransform::from_rotation(Rot2::radians(angle)),
                        BackgroundColor(colors::ON_PRIMARY),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            FocusPolicy::Pass,
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(1.),
                                top: px(1.),
                                width: px(8.),
                                height: px(2.),
                                ..default()
                            },
                            BackgroundColor(colors::ON_SURFACE),
                        ));
                    });
            }
        });
}

/// 当帧投影分类：画面内十字锚点、画面外边缘箭头或无效隐藏。
#[derive(Debug, PartialEq, Clone, Copy)]
pub(crate) enum Placement {
    /// 画面内：视口局部逻辑像素中心，即投影点本身。
    Inside(Vec2),
    /// 画面外：箭头锚点（视口局部逻辑像素）与指向角（弧度，y 向下屏幕空间，
    /// 0 表示指向 +X）。角度取自视口中心到投影点的方向，只表示调炮方向。
    Edge(Vec2, f32),
}

/// 使用完整相机视口的逻辑坐标把世界点分为三态：画面内、画面外或无效。
///
/// 相机后方、近裁剪面内、远裁剪面外及非有限值一律返回 `None`；转换只依赖
/// 相机自身的逻辑视口与显示缩放，不叠加菜单整体缩放。
pub(crate) fn project(
    camera: &Camera,
    transform: &GlobalTransform,
    point: Vec3,
) -> Option<Placement> {
    if !point.is_finite() || !transform.affine().is_finite() {
        return None;
    }
    let viewport = camera.logical_viewport_rect()?;
    let ndc = camera.world_to_ndc(transform, point)?;
    if !ndc.is_finite() || ndc.z <= 0.0 || ndc.z >= 1.0 {
        return None;
    }
    let position = camera.world_to_viewport(transform, point).ok()?;
    let local = position - viewport.min;
    if !local.is_finite() {
        return None;
    }
    if local.cmpge(Vec2::ZERO).all() && local.cmple(viewport.size()).all() {
        return Some(Placement::Inside(local));
    }
    // 锚点钳制使 16px 箭头整体距视口边缘 ≥24 逻辑像素；视口放不下时隐藏。
    let inset = EDGE_MARGIN + ARROW_SIZE / 2.;
    if viewport.size().cmplt(Vec2::splat(inset * 2.)).any() {
        return None;
    }
    // 画面外点到视口中心的方向必非退化；y 向下空间直接取 atan2。
    let direction = local - viewport.size() / 2.;
    Some(Placement::Edge(
        local.clamp(Vec2::splat(inset), viewport.size() - Vec2::splat(inset)),
        direction.y.atan2(direction.x),
    ))
}

/// 每次先隐藏全部标记，任何资源或投影失败都不会留下上一帧状态。
fn update(
    mut commands: Commands,
    prediction: Res<ShotPrediction>,
    scale: Res<UiScale>,
    cameras: Query<(Entity, &Camera, &GlobalTransform), With<FollowCamera>>,
    mut roots: Query<(Entity, &mut Node, &mut UiTransform), (With<Reticle>, Without<EdgeArrow>)>,
    mut arrows: Query<(Entity, &mut Node, &mut UiTransform), (With<EdgeArrow>, Without<Reticle>)>,
    mut rings: Query<&mut Node, (With<HitRing>, Without<Reticle>, Without<EdgeArrow>)>,
) {
    for (_, mut node, _) in &mut roots {
        node.display = Display::None;
    }
    for (_, mut node, _) in &mut arrows {
        node.display = Display::None;
    }
    for mut ring in &mut rings {
        ring.display = Display::None;
    }
    let (Some(shot), Ok((camera_entity, camera, transform))) = (prediction.0, cameras.single())
    else {
        return;
    };
    if !scale.0.is_finite() || scale.0 <= 0.0 {
        return;
    }
    match project(camera, transform, shot.point) {
        Some(Placement::Inside(center)) => {
            // UiScale 仅在这里抵消一次；游戏菜单比例不改变准星的逻辑像素尺寸。
            for (entity, mut node, mut ui_transform) in &mut roots {
                ui_transform.scale = Vec2::splat(1.0 / scale.0);
                node.left = px(center.x / scale.0 - 14.);
                node.top = px(center.y / scale.0 - 14.);
                node.display = Display::Flex;
                commands.entity(entity).insert(UiTargetCamera(camera_entity));
            }
            if shot.target.is_some() {
                for mut ring in &mut rings {
                    ring.display = Display::Flex;
                }
            }
        }
        Some(Placement::Edge(anchor, angle)) => {
            for (entity, mut node, mut ui_transform) in &mut arrows {
                ui_transform.scale = Vec2::splat(1.0 / scale.0);
                ui_transform.rotation = Rot2::radians(angle);
                node.left = px(anchor.x / scale.0 - ARROW_SIZE / 2.);
                node.top = px(anchor.y / scale.0 - ARROW_SIZE / 2.);
                node.display = Display::Flex;
                commands.entity(entity).insert(UiTargetCamera(camera_entity));
            }
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::{CameraProjection, RenderTargetInfo};

    /// 独立对称透视相机：`width`/`height` 为逻辑像素，`scale_factor` 为显示缩放。
    fn test_camera(width: f32, height: f32, scale_factor: f32) -> Camera {
        let mut camera = Camera::default();
        camera.computed.clip_from_view = PerspectiveProjection {
            fov: std::f32::consts::FRAC_PI_2,
            aspect_ratio: width / height,
            near: 0.1,
            ..default()
        }
        .get_clip_from_view();
        camera.computed.target_info = Some(RenderTargetInfo {
            physical_size: UVec2::new(
                (width * scale_factor) as u32,
                (height * scale_factor) as u32,
            ),
            scale_factor,
        });
        camera
    }

    /// 画面内投影在 16:9、较窄、超宽及非 1 显示缩放下都等于逻辑视口中心，
    /// 且误差远小于 2 逻辑像素；缩放只由相机完成一次。
    #[test]
    fn in_view_projection_is_exact_across_viewports_and_scales() {
        for (width, height, scale) in [
            (1280., 720., 1.),
            (960., 720., 1.),
            (2560., 1080., 1.),
            (1280., 720., 1.5),
        ] {
            let camera = test_camera(width, height, scale);
            let transform = GlobalTransform::IDENTITY;
            match project(&camera, &transform, Vec3::new(0., 0., -10.)) {
                Some(Placement::Inside(center)) => assert!(
                    center
                        .abs_diff_eq(Vec2::new(width / 2., height / 2.), 1e-3),
                    "{width}x{height}@{scale}：{center:?}"
                ),
                other => panic!("前方中心点应为画面内：{other:?}"),
            }
        }
    }

    /// 画面外四边与四角：锚点被钳制到距边缘 ≥24px 且整体在视口内，角度指向
    /// 投影点相对视口中心的实际方向（用相机 API 独立复算，不用被测函数）。
    #[test]
    fn edge_arrow_is_clamped_inside_and_points_toward_projection() {
        let camera = test_camera(1280., 720., 1.);
        let transform = GlobalTransform::IDENTITY;
        let viewport = camera.logical_viewport_rect().unwrap();
        let inset = EDGE_MARGIN + ARROW_SIZE / 2.;
        for point in [
            Vec3::new(-100., 0., -10.),
            Vec3::new(100., 0., -10.),
            Vec3::new(0., 100., -10.),
            Vec3::new(0., -100., -10.),
            Vec3::new(100., 100., -10.),
            Vec3::new(-100., -100., -10.),
            Vec3::new(100., -100., -10.),
            Vec3::new(-100., 100., -10.),
        ] {
            let Some(Placement::Edge(anchor, angle)) = project(&camera, &transform, point) else {
                panic!("{point:?} 应判为画面外边缘箭头");
            };
            // 整个 16px 标记留在视口内且距边缘至少 24 逻辑像素。
            assert!(anchor.is_finite() && angle.is_finite(), "{point:?} 坐标有限");
            assert!(
                anchor.cmpge(Vec2::splat(inset)).all()
                    && anchor.cmple(viewport.size() - Vec2::splat(inset)).all(),
                "{point:?} 锚点越界：{anchor:?}"
            );
            // 指向角与相机 API 投影方向一致，只表示调炮方向。
            let local = camera.world_to_viewport(&transform, point).unwrap() - viewport.min;
            let expected = (local - viewport.size() / 2.).y.atan2((local - viewport.size() / 2.).x);
            assert!(
                (angle - expected).abs() < 1e-4,
                "{point:?} 指向角 {angle} ≠ {expected}"
            );
        }
        // 方向语义抽查：上方点箭头朝上（-π/2），右方点朝 +X（0）。
        for (point, angle) in [
            (Vec3::new(0., 100., -10.), -std::f32::consts::FRAC_PI_2),
            (Vec3::new(100., 0., -10.), 0.),
        ] {
            match project(&camera, &transform, point) {
                Some(Placement::Edge(_, actual)) => {
                    assert!((actual - angle).abs() < 1e-4, "{point:?} 指向 {actual}")
                }
                other => panic!("{point:?} 应为边缘箭头：{other:?}"),
            }
        }
    }

    /// 相机后方、近裁剪面内、非有限及无视口信息一律隐藏，
    /// 不产生锚点或屏幕中心兜底。
    #[test]
    fn projection_hides_invalid_points_without_a_center_fallback() {
        let camera = test_camera(1280., 720., 1.);
        let transform = GlobalTransform::IDENTITY;
        for point in [
            Vec3::Z,
            Vec3::new(0., 0., -0.05),
            Vec3::splat(f32::NAN),
            Vec3::new(f32::INFINITY, 0., -10.),
        ] {
            assert_eq!(project(&camera, &transform, point), None, "{point:?}");
        }
        let mut camera = camera;
        camera.computed.target_info = None;
        assert_eq!(project(&camera, &transform, Vec3::NEG_Z), None);
    }
}
