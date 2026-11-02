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

/// 实际跟随相机未出现时保持隐藏，后续逐帧绑定。
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
}

/// 使用完整相机视口的逻辑坐标，近面/后方/无效及画面外暂时隐藏。
pub(crate) fn project(camera: &Camera, transform: &GlobalTransform, point: Vec3) -> Option<Vec2> {
    if !point.is_finite() || !transform.affine().is_finite() {
        return None;
    }
    let viewport = camera.logical_viewport_rect()?;
    let ndc = camera.world_to_ndc(transform, point)?;
    if !ndc.is_finite() || ndc.z <= 0.0 || ndc.z >= 1.0 {
        return None;
    }
    let position = camera.world_to_viewport(transform, point).ok()?;
    // 标记整体留在视口内；边缘提示由工单 03 实现。
    let local = position - viewport.min;
    (local.is_finite()
        && local.cmpge(Vec2::splat(14.)).all()
        && local.cmple(viewport.size() - Vec2::splat(14.)).all())
    .then_some(local)
}

/// 每次先隐藏，任何资源或投影失败都不会留下上一帧命中。
fn update(
    mut commands: Commands,
    prediction: Res<ShotPrediction>,
    scale: Res<UiScale>,
    cameras: Query<(Entity, &Camera, &GlobalTransform), With<FollowCamera>>,
    mut roots: Query<(Entity, &mut Node, &mut UiTransform), With<Reticle>>,
    mut rings: Query<&mut Node, (With<HitRing>, Without<Reticle>)>,
) {
    for (entity, mut node, mut ui_transform) in &mut roots {
        node.display = Display::None;
        let (Some(shot), Ok((camera_entity, camera, transform))) = (prediction.0, cameras.single())
        else {
            continue;
        };
        let Some(center) = project(camera, transform, shot.point) else {
            continue;
        };
        if !scale.0.is_finite() || scale.0 <= 0.0 {
            continue;
        }
        // UiScale 仅在这里抵消一次；游戏菜单比例不改变准星的逻辑像素尺寸。
        ui_transform.scale = Vec2::splat(1.0 / scale.0);
        node.left = px(center.x / scale.0 - 14.);
        node.top = px(center.y / scale.0 - 14.);
        node.display = Display::Flex;
        commands
            .entity(entity)
            .insert(UiTargetCamera(camera_entity));
        for mut ring in &mut rings {
            ring.display = if shot.target.is_some() {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::{CameraProjection, RenderTargetInfo};

    /// 独立对称透视几何：前方中心居中，后方、近面、非有限及四边之外隐藏。
    #[test]
    fn projection_hides_invalid_points_without_a_center_fallback() {
        let mut camera = Camera::default();
        camera.computed.clip_from_view = PerspectiveProjection {
            fov: std::f32::consts::FRAC_PI_2,
            aspect_ratio: 1280. / 720.,
            near: 0.1,
            ..default()
        }
        .get_clip_from_view();
        camera.computed.target_info = Some(RenderTargetInfo {
            physical_size: UVec2::new(1280, 720),
            scale_factor: 1.,
        });
        let transform = GlobalTransform::IDENTITY;
        assert_eq!(
            project(&camera, &transform, Vec3::new(0., 0., -10.)),
            Some(Vec2::new(640., 360.))
        );
        for point in [
            Vec3::Z,
            Vec3::new(0., 0., -0.05),
            Vec3::splat(f32::NAN),
            Vec3::new(100., 0., -10.),
            Vec3::new(-100., 0., -10.),
            Vec3::new(0., 100., -10.),
            Vec3::new(0., -100., -10.),
        ] {
            assert_eq!(project(&camera, &transform, point), None);
        }
        camera.computed.target_info = None;
        assert_eq!(project(&camera, &transform, Vec3::NEG_Z), None);
    }
}
