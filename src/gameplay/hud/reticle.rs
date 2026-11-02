//! 对局中固定在游戏视口中心的方向十字。

use crate::{
    app::state::GameState,
    gameplay::{camera::FollowCamera, session::SessionEntity},
    ui::resources::colors,
};
use bevy::{
    camera::CameraUpdateSystems,
    prelude::*,
    ui::{FocusPolicy, UiSystems},
};

/// 28 像素根框包含 20 像素十字及描边，不提示命中状态。
#[derive(Component)]
pub(crate) struct Reticle;

/// 生成与定位对局十字。
pub(super) struct ReticlePlugin;
impl Plugin for ReticlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), setup)
            .add_systems(
                PostUpdate,
                update.after(CameraUpdateSystems).before(UiSystems::Prepare),
            );
    }
}

/// 十字随对局清理。
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

/// 抵消全局 UI 缩放，保持十字尺寸及视口中心的逻辑像素位置。
fn update(
    mut commands: Commands,
    scale: Res<UiScale>,
    cameras: Query<(Entity, &Camera), With<FollowCamera>>,
    mut roots: Query<(Entity, &mut Node, &mut UiTransform), With<Reticle>>,
) {
    for (entity, mut node, mut transform) in &mut roots {
        node.display = Display::None;
        let Ok((camera_entity, camera)) = cameras.single() else {
            continue;
        };
        let Some(rect) = camera.logical_viewport_rect() else {
            continue;
        };
        if !scale.0.is_finite() || scale.0 <= 0.0 {
            continue;
        }
        let center = rect.size() / 2.0;
        transform.scale = Vec2::splat(1.0 / scale.0);
        node.left = px(center.x / scale.0 - 14.0);
        node.top = px(center.y / scale.0 - 14.0);
        node.display = Display::Flex;
        commands
            .entity(entity)
            .insert(UiTargetCamera(camera_entity));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::RenderTargetInfo;

    /// 不同视口和显示缩放下，UI 缩放前的定位仍锚定逻辑视口中心。
    #[test]
    fn reticle_centers_for_viewports_and_ui_scales() {
        for (width, height, display_scale, ui_scale) in [
            (1280., 720., 1., 1.),
            (640., 720., 1., 0.5),
            (1920., 720., 1.5, 1.),
        ] {
            let mut app = App::new();
            app.insert_resource(UiScale(ui_scale))
                .add_systems(Update, update);
            let mut camera = Camera::default();
            camera.computed.target_info = Some(RenderTargetInfo {
                physical_size: UVec2::new(
                    (width * display_scale) as u32,
                    (height * display_scale) as u32,
                ),
                scale_factor: display_scale,
            });
            app.world_mut().spawn((FollowCamera, camera));
            let reticle = app
                .world_mut()
                .spawn((Reticle, Node::default(), UiTransform::default()))
                .id();
            app.update();
            let node = app.world().get::<Node>(reticle).unwrap();
            assert_eq!(node.left, px(width / (2. * ui_scale) - 14.));
            assert_eq!(node.top, px(height / (2. * ui_scale) - 14.));
            assert_eq!(node.display, Display::Flex);
            assert_eq!(
                app.world().get::<UiTransform>(reticle).unwrap().scale,
                Vec2::splat(1. / ui_scale)
            );
        }
    }
}
