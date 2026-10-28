//! 真实 VOX 模型画廊：纯视觉预览与页面生命周期，不参与战斗和物理。
pub(crate) mod ui;

use crate::app::{
    state::GameState,
    transition::{Fade, SceneTarget},
};
use bevy::{camera::visibility::RenderLayers, prelude::*};

use super::model_preview::{self as preview, prepare_previews, spawn_previews, spin};
pub(crate) use super::model_preview::{ENTRIES, Previews, validate};
#[cfg(test)]
use super::model_preview::{Entry, assets_ready, model_nodes};

/// 页面相机复用菜单的纹理缩放规则，不参与模型预览层。
#[derive(Component)]
pub(crate) struct GalleryCamera;
/// 画廊持有的根实体，离开时递归清理。
#[derive(Component)]
pub(crate) struct GalleryEntity;

/// 安装页面及纯视觉预览系统。
pub(crate) struct GalleryPlugin;
impl Plugin for GalleryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ui::View>()
            .add_systems(
                OnEnter(GameState::Gallery),
                (spawn_previews, ui::spawn).chain(),
            )
            .add_systems(Update, (prepare_previews, spin).chain().run_if(in_gallery))
            .add_systems(
                Update,
                (
                    ui::input.before(crate::ui::widgets::title_bar::interaction),
                    ui::update,
                )
                    .chain()
                    .run_if(in_gallery),
            )
            .add_systems(
                OnExit(GameState::GalleryToIndex),
                (cleanup, preview::cleanup),
            )
            .add_systems(
                OnExit(GameState::GalleryToDetail),
                (cleanup, preview::cleanup),
            );
    }
}

/// 渐出时保留最后一页内容与动画，黑场交接后才销毁。
fn in_gallery(state: Res<State<GameState>>) -> bool {
    matches!(
        state.get(),
        GameState::Gallery | GameState::GalleryToIndex | GameState::GalleryToDetail
    )
}

/// 供离屏验证操作同一指针与滚动状态，不绕过页面输入处理。
pub(crate) fn view_mut(world: &mut World) -> Mut<'_, ui::View> {
    world.resource_mut::<ui::View>()
}

/// 黑场交接时删除全部页面根，并释放预览专用纹理。
fn cleanup(
    mut commands: Commands,
    roots: Query<Entity, With<GalleryEntity>>,
    title: Option<ResMut<crate::ui::widgets::TitleBarState>>,
) {
    if let Some(mut title) = title {
        title.external_navigation = false;
        title.set_focus(false);
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 清单只包含当前真实型号，整车型号来自车身目录。
    #[test]
    fn catalog_uses_real_variant_directories() {
        assert_eq!(
            ENTRIES.map(Entry::name),
            [
                "lance", "spit", "cleat", "crawl", "anvil", "carapace", "anvil", "carapace", "concrete"
            ]
        );
        for category in 0..4 {
            assert_eq!(ENTRIES.iter().filter(|e| e.category == category).count(), 2);
        }
        assert_eq!(ENTRIES.iter().filter(|e| e.category == 4).count(), 1);
        for entry in ENTRIES {
            assert!(std::path::Path::new("assets").join(entry.path).is_file());
        }
    }

    /// 无 GPU 和窗口，使用真实加载器验收所有预览的几何、轮组及资源清理。
    #[test]
    fn real_previews_measure_visible_geometry_and_clean_instances() {
        use bevy::{
            asset::AssetApp,
            camera::ScalingMode,
            camera::primitives::MeshAabb,
            camera::visibility::VisibilityClass,
            ecs::system::RunSystemOnce,
            light::FogVolume,
            world_serialization::{WorldInstance, WorldSerializationPlugin},
        };
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            ImagePlugin::default(),
            WorldSerializationPlugin,
            TransformPlugin,
            // 单位来自资产旁 .vox.meta（global_settings 覆盖会掩盖缺配置）。
            bevy_vox_scene::VoxScenePlugin::default(),
            crate::tank::track::TrackAnimationPlugin,
        ))
        .init_asset::<StandardMaterial>()
        .init_asset::<Mesh>()
        .register_type::<Visibility>()
        .register_type::<ViewVisibility>()
        .register_type::<InheritedVisibility>()
        .register_type::<VisibilityClass>()
        .register_type::<Transform>()
        .register_type::<GlobalTransform>()
        .register_type::<TransformTreeChanged>()
        .register_type::<Mesh3d>()
        .register_type::<MeshMaterial3d<StandardMaterial>>()
        .register_type::<FogVolume>()
        .add_systems(Startup, spawn_previews)
        .add_systems(Update, prepare_previews);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            app.update();
            if assets_ready(app.world()) {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "真实预览加载超时");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        validate(app.world()).unwrap();
        // X/Y/Z 为长/高/宽；这些边界来自真实体素，不能替换为共同画布或碰撞盒。
        let expected = [
            Vec3::new(4.5, 0.4, 0.4),
            Vec3::new(3.8, 0.4, 0.4),
            Vec3::new(7.5, 1.3, 3.6),
            Vec3::new(6.6, 1.4, 3.4),
            Vec3::new(7.5, 2.2, 3.4),
            Vec3::new(6.5, 2.1, 3.2),
            Vec3::new(10.5, 2.6, 3.6),
            Vec3::new(9.0, 2.6, 3.4),
            // concrete 资产的体素画布改版后未同步预期；以真实体素 AABB 为准。
            Vec3::new(8.0, 8.0, 1.5),
        ];
        for (index, (preview, size)) in app
            .world()
            .resource::<Previews>()
            .items
            .iter()
            .zip(expected)
            .enumerate()
        {
            assert!(
                preview.size.unwrap().abs_diff_eq(size, 0.001),
                "{:?} != {size:?}",
                preview.size
            );
            if ENTRIES[index].category == 3 {
                assert_eq!(
                    app.world()
                        .get::<Transform>(preview.content)
                        .unwrap()
                        .translation,
                    Vec3::ZERO,
                    "整车必须绕 body 根节点原点旋转，不能按包含炮管的包围盒重新居中"
                );
                let camera = app.world().get::<Transform>(preview.camera).unwrap();
                let Projection::Orthographic(projection) =
                    app.world().get::<Projection>(preview.camera).unwrap()
                else {
                    panic!("预览应使用正交相机");
                };
                let ScalingMode::FixedVertical { viewport_height } = projection.scaling_mode else {
                    panic!("预览应按竖直视野适配");
                };
                let image_size = app
                    .world()
                    .resource::<Assets<Image>>()
                    .get(&preview.image)
                    .unwrap()
                    .size()
                    .as_vec2();
                let half_height = viewport_height * 0.5;
                let half_width = half_height * image_size.x / image_size.y;
                // 真实网格边界转过完整一周，在相机空间检查四边，覆盖长炮管的扫掠范围。
                for (entity, affine, visible, _) in model_nodes(app.world(), preview.content) {
                    if !visible {
                        continue;
                    }
                    let Some(mesh) = app.world().get::<Mesh3d>(entity) else {
                        continue;
                    };
                    let aabb = app
                        .world()
                        .resource::<Assets<Mesh>>()
                        .get(&mesh.0)
                        .unwrap()
                        .get_aabb()
                        .unwrap();
                    for x in [-1.0, 1.0] {
                        for y in [-1.0, 1.0] {
                            for z in [-1.0, 1.0] {
                                let point = affine.transform_point3(
                                    Vec3::from(aabb.center)
                                        + Vec3::from(aabb.half_extents) * Vec3::new(x, y, z),
                                );
                                for degrees in (0..360).step_by(15) {
                                    let rotated =
                                        Quat::from_rotation_y((degrees as f32).to_radians())
                                            * point;
                                    let view_point =
                                        camera.rotation.inverse() * (rotated - camera.translation);
                                    assert!(
                                        view_point.x.abs() < half_width
                                            && view_point.y.abs() < half_height
                                            && view_point.z < 0.0,
                                        "{} 自转时裁切：{degrees}° {view_point:?}",
                                        ENTRIES[index].name()
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        let instances: Vec<_> = app
            .world_mut()
            .query::<&WorldInstance>()
            .iter(app.world())
            .map(|i| **i)
            .collect();
        // 85 个模型实例 + lance 伴随子弹 1 个场景实例。
        assert_eq!(instances.len(), 86);
        // 模拟真实文件缺失，不修改任何资产；失败项必须结束等待并保留返回能力。
        let missing = app
            .world()
            .resource::<AssetServer>()
            .load("tank/barrels/player/__missing_gallery_test__/barrel.vox");
        let content = app.world().resource::<Previews>().items[0].content;
        app.world_mut().resource_mut::<Previews>().items[0].size = None;
        app.world_mut()
            .entity_mut(content)
            .insert(WorldAssetRoot(missing));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.world().resource::<Previews>().items[0].error.is_none() {
            app.update();
            assert!(std::time::Instant::now() < deadline, "失败资产没有结束等待");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(assets_ready(app.world()));
        assert!(validate(app.world()).is_err());
        app.world_mut().run_system_once(cleanup).unwrap();
        app.world_mut().run_system_once(preview::cleanup).unwrap();
        app.update();
        assert!(!app.world().contains_resource::<Previews>());
        assert!(instances.iter().all(|id| {
            !app.world()
                .resource::<WorldInstanceSpawner>()
                .instance_is_ready(*id)
        }));
    }
}
